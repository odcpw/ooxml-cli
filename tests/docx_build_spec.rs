use ooxml_cli::build::{BuildFamily, compile_docx_spec, load_spec_file, load_spec_str};
use regex::Regex;
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use zip::ZipArchive;

const REPORT_SPEC: &str = "testdata/docx/build-spec/quarterly-report.json";
const REPORT_GOLDEN: &str = "testdata/golden/build-spec/docx/quarterly-report-summary.json";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn json_stdout(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "JSON stdout ({error}): {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label} failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn temp_dir(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "ooxml-docx-build-spec-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create test directory");
    path
}

fn path(path: &Path) -> &str {
    path.to_str().expect("UTF-8 test path")
}

#[test]
fn quarterly_report_compiles_to_the_expected_atomic_operation_sequence() {
    let spec = load_spec_file(BuildFamily::Docx, REPORT_SPEC).expect("load quarterly report spec");
    let compiled = compile_docx_spec(&spec).expect("compile quarterly report spec");
    let commands = compiled
        .plan
        .operations
        .iter()
        .map(|operation| operation.command.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        commands,
        [
            "docx scaffold",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx fields insert",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx tables create",
            "docx images insert",
            "docx breaks insert",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx paragraphs append",
            "docx blocks delete",
            "docx sections set",
            "docx headers set-text",
            "docx footers set-text",
        ]
    );
    assert_eq!(
        compiled.plan.operations[16].args["expectHash"],
        json!({"$ref": "document.readback.blockHashes.0.contentHash"})
    );
    assert_eq!(
        compiled.plan.operations[5].args["runs"][2]["inlineCode"],
        true
    );
    assert_eq!(compiled.plan.operations[6].args["list"], "bullet");
    assert_eq!(compiled.plan.operations[7].args["level"], 1);
    assert_eq!(compiled.plan.operations[9].args["style"], "TableLight");
    assert_eq!(
        compiled.plan.operations[9].args["widths"],
        "2.3in,1.2in,1in"
    );
    assert_eq!(
        compiled.plan.operations[10].args["caption"],
        "Product adoption accelerated throughout Q3"
    );
    assert_eq!(
        compiled.plan.operations[17].args["margins"],
        "0.75in,0.8in,0.75in,0.8in"
    );
    assert_eq!(compiled.plan.operations[19].args["pageNumbers"], true);
    assert_eq!(
        compiled.plan.node_map["/blocks/9"].spec_id.as_deref(),
        Some("product-image")
    );
}

#[test]
fn quarterly_report_build_is_strict_sdk_clean_renderable_and_byte_deterministic() {
    let temp = temp_dir("quarterly-report");
    let first = temp.join("quarterly-report.docx");
    let second = temp.join("quarterly-report-copy.docx");
    let built = run(&[
        "--json",
        "docx",
        "build",
        "--spec",
        REPORT_SPEC,
        "--out",
        path(&first),
        "--check",
    ]);
    assert_success(&built, "quarterly report build");
    let report = json_stdout(&built);
    assert_eq!(report["schemaVersion"], "ooxml-cli.docx-build.v1");
    assert_eq!(report["validated"], true);
    assert_eq!(report["mutationEnvelope"]["opsCount"], 20);
    assert_eq!(report["check"]["summary"]["errors"], 0);
    assert_eq!(report["outline"]["summary"]["tables"], 1);
    assert_eq!(report["outline"]["summary"]["mediaAssets"], 1);
    assert_eq!(report["outline"]["summary"]["headers"], 1);
    assert_eq!(report["outline"]["summary"]["footers"], 1);
    assert_eq!(report["outline"]["summary"]["hyperlinks"], 1);
    assert_eq!(
        report["outline"]["coreProperties"]["title"],
        "Q3 quarterly report"
    );
    assert_eq!(
        report["outline"]["coreProperties"]["subject"],
        "Quarterly operating review"
    );
    assert_golden(&semantic_summary(&report));

    let blocks = report["outline"]["blocks"]
        .as_array()
        .expect("outline blocks");
    assert_eq!(blocks[0]["styleId"], "Title");
    assert_eq!(blocks[1]["styleId"], "Subtitle");
    assert_eq!(blocks[3]["styleId"], "Heading1");
    assert_eq!(blocks[7]["styleId"], "Heading2");
    assert_eq!(blocks[5]["styleId"], "ListBullet");
    assert_eq!(blocks[5]["listLevel"], 0);
    assert_eq!(blocks[6]["listLevel"], 1);
    assert_eq!(blocks[14]["styleId"], "ListNumber");
    assert!(
        blocks[4]["textPreview"].as_str().is_some_and(|text| {
            text.contains("Revenue grew 18%") && text.contains("forecast()")
        })
    );
    let fields = report["outline"]["fields"]
        .as_array()
        .expect("outline fields");
    for instruction in ["TOC", "SEQ Figure", "PAGE", "NUMPAGES"] {
        assert!(
            fields.iter().any(|field| field["instruction"]
                .as_str()
                .is_some_and(|actual| actual.starts_with(instruction))),
            "missing {instruction} field: {fields:?}"
        );
    }

    assert_docx_proofs(&first);
    let document_xml = zip_text(&first, "word/document.xml");
    assert!(document_xml.contains("w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\""));
    assert!(document_xml.contains("w:shd w:val=\"clear\" w:fill=\"F2F2F2\""));
    assert!(document_xml.contains("w:rStyle w:val=\"Hyperlink\""));
    assert!(document_xml.contains("w:tblHeader"));
    assert!(document_xml.contains("TOC \\o &quot;1-4&quot; \\h \\z \\u"));
    let relationships = zip_text(&first, "word/_rels/document.xml.rels");
    assert!(relationships.contains("Target=\"https://example.com/q3\" TargetMode=\"External\""));
    let settings = zip_text(&first, "word/settings.xml");
    assert!(settings.contains("w:updateFields w:val=\"true\""));
    let footer = zip_text(&first, "word/footer1.xml");
    assert!(footer.contains("PAGE") && footer.contains("NUMPAGES"));

    assert_libreoffice_heading_render(&temp, &first);
    let second_build = run(&[
        "--json",
        "docx",
        "build",
        "--spec",
        REPORT_SPEC,
        "--out",
        path(&second),
    ]);
    assert_success(&second_build, "second quarterly report build");
    assert_eq!(
        fs::read(&first).expect("read first DOCX"),
        fs::read(&second).expect("read second DOCX"),
        "identical DOCX specs must produce byte-identical packages"
    );
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn dry_run_executes_and_validates_the_complete_docx_batch_without_publishing() {
    let temp = temp_dir("dry-run");
    let output = temp.join("planned.docx");
    let dry_run = run(&[
        "--json",
        "docx",
        "build",
        "--spec",
        REPORT_SPEC,
        "--out",
        path(&output),
        "--dry-run",
    ]);
    assert_success(&dry_run, "DOCX build dry-run");
    let result = json_stdout(&dry_run);
    assert_eq!(result["dryRun"], true);
    assert_eq!(result["validated"], true);
    assert_eq!(result["mutationEnvelope"]["committed"], false);
    assert_eq!(result["mutationEnvelope"]["opsCount"], 20);
    assert_eq!(result["outline"], Value::Null);
    assert!(!output.exists());
    let serialized = serde_json::to_string(&result).expect("serialize dry-run report");
    assert!(serialized.contains("<build-stage>"));
    assert!(!serialized.contains(&format!("ooxml-docx-build-{}", std::process::id())));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn table_blocks_materialize_inline_csv_json_and_xlsx_range_sources() {
    let temp = temp_dir("table-sources");
    let csv = temp.join("source.csv");
    let json_table = temp.join("source.json");
    fs::write(&csv, "Source,Value\nCSV,2\n").expect("write CSV source");
    fs::write(&json_table, "[[\"Source\",\"Value\"],[\"JSON\",3]]\n").expect("write JSON source");

    let xlsx = temp.join("source.xlsx");
    let scaffold = run(&["--json", "xlsx", "scaffold", path(&xlsx), "--force"]);
    assert_success(&scaffold, "source workbook scaffold");
    let seeded = temp.join("source-seeded.xlsx");
    let set = run(&[
        "--json",
        "xlsx",
        "ranges",
        "set",
        path(&xlsx),
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--values",
        "[[\"Source\",\"Value\"],[\"XLSX\",4]]",
        "--out",
        path(&seeded),
    ]);
    assert_success(&set, "seed source workbook");

    let spec = temp.join("tables.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "family": "docx",
            "blocks": [
                {"type": "table", "rows": [["Source", "Value"], ["Inline", 1]]},
                {"type": "table", "table": {"csv": "source.csv", "style": "TableGrid"}},
                {"type": "table", "table": {"json": "source.json", "style": "TableGrid"}},
                {"type": "table", "table": {
                    "xlsx": {"path": "source-seeded.xlsx", "sheet": "Sheet1", "range": "A1:B2"},
                    "style": "TableGrid"
                }}
            ]
        }))
        .expect("serialize table spec"),
    )
    .expect("write table spec");
    let output = temp.join("tables.docx");
    let built = run(&[
        "--json",
        "docx",
        "build",
        "--spec",
        path(&spec),
        "--out",
        path(&output),
    ]);
    assert_success(&built, "source-backed DOCX table build");
    let report = json_stdout(&built);
    assert_eq!(report["outline"]["summary"]["tables"], 4);
    let text = report["outline"]["blocks"]
        .as_array()
        .expect("outline blocks")
        .iter()
        .filter_map(|block| block["textPreview"].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    for source in ["Inline", "CSV", "JSON", "XLSX"] {
        assert!(text.contains(source), "missing {source} table in {text:?}");
    }
    assert_docx_proofs(&output);
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn invalid_heading_and_dangling_style_fail_with_actionable_paths_and_no_output() {
    let invalid_heading = load_spec_str(
        BuildFamily::Docx,
        &json!({
            "schemaVersion": 1,
            "family": "docx",
            "blocks": [{"type": "heading", "level": 5, "text": "Too deep"}]
        })
        .to_string(),
    )
    .expect("schema accepts an integer level for compiler validation");
    let error = compile_docx_spec(&invalid_heading).expect_err("heading 5 must fail");
    assert_eq!(error.code, "BUILD_SPEC_VALUE_INVALID");
    assert_eq!(error.path, "/blocks/0/level");
    assert!(error.message.contains("1 through 4"));

    let temp = temp_dir("invalid-style");
    let spec = temp.join("invalid-style.json");
    fs::write(
        &spec,
        serde_json::to_vec_pretty(&json!({
            "schemaVersion": 1,
            "family": "docx",
            "blocks": [{"type": "paragraph", "style": "MissingStyle", "text": "No dangling references"}]
        }))
        .expect("serialize invalid style spec"),
    )
    .expect("write invalid style spec");
    let output = temp.join("must-not-exist.docx");
    let failed = run(&[
        "--json",
        "docx",
        "build",
        "--spec",
        path(&spec),
        "--out",
        path(&output),
    ]);
    assert_eq!(failed.status.code(), Some(6));
    let error: Value = serde_json::from_slice(&failed.stderr).expect("JSON error on stderr");
    assert!(
        error["error"]["message"].as_str().is_some_and(
            |message| message.contains("style not found: \"MissingStyle\" (paragraph)")
        )
    );
    assert!(!output.exists(), "failed atomic build published an output");
    let _ = fs::remove_dir_all(temp);
}

fn semantic_summary(result: &Value) -> Value {
    let styles = result["outline"]["blocks"]
        .as_array()
        .expect("outline blocks")
        .iter()
        .filter_map(|block| block["styleId"].as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let fields = result["outline"]["fields"]
        .as_array()
        .expect("outline fields")
        .iter()
        .filter_map(|field| field["instruction"].as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let node_ids = result["compiledPlan"]["nodeMap"]
        .as_object()
        .expect("compiled node map")
        .values()
        .filter_map(|node| node["specId"].as_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let operations = result["compiledPlan"]["operations"]
        .as_array()
        .expect("compiled operations")
        .iter()
        .map(|operation| operation["command"].clone())
        .collect::<Vec<_>>();
    json!({
        "schemaVersion": result["schemaVersion"],
        "validated": result["validated"],
        "checkErrors": result["check"]["summary"]["errors"],
        "nodeIds": node_ids,
        "operations": operations,
        "styles": styles,
        "fields": fields,
        "summary": result["outline"]["summary"],
        "coreProperties": result["outline"]["coreProperties"],
    })
}

fn assert_golden(actual: &Value) {
    let mut rendered = serde_json::to_string_pretty(actual).expect("serialize report summary");
    rendered.push('\n');
    if std::env::var("UPDATE_GOLDENS").as_deref() == Ok("1") {
        fs::create_dir_all(Path::new(REPORT_GOLDEN).parent().expect("golden parent"))
            .expect("create golden directory");
        fs::write(REPORT_GOLDEN, &rendered).expect("update reviewed report summary golden");
    }
    let expected = fs::read_to_string(REPORT_GOLDEN).unwrap_or_else(|error| {
        panic!("missing {REPORT_GOLDEN}: {error}; rerun with UPDATE_GOLDENS=1")
    });
    assert_eq!(
        rendered, expected,
        "quarterly report semantic summary drifted"
    );
}

fn assert_docx_proofs(document: &Path) {
    let strict = run(&["--json", "validate", "--strict", path(document)]);
    assert_success(&strict, "strict DOCX validation");
    let conformance = run(&["--json", "conformance", "check", path(document)]);
    assert_success(&conformance, "DOCX conformance validation");
    let conformance = json_stdout(&conformance);
    assert_eq!(conformance["summary"]["errors"], 0, "{conformance}");
    assert_eq!(conformance["summary"]["warnings"], 0, "{conformance}");
    assert!(
        conformance["checks"]
            .as_array()
            .expect("conformance checks")
            .iter()
            .any(|check| check["name"] == "repo-validation" && check["status"] == "passed")
    );
    assert_openxml_sdk_clean(document);
}

fn assert_openxml_sdk_clean(document: &Path) {
    let dotnet = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("dotnet/dotnet"));
    let validator = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tools/openxml-validator/bin/Release/net8.0/openxml-validator.dll");
    if !dotnet.as_deref().is_some_and(Path::is_file) || !validator.is_file() {
        println!(
            "SKIP Open XML SDK DOCX build proof: ~/dotnet/dotnet or {} is unavailable",
            validator.display()
        );
        assert_ne!(
            std::env::var("OOXML_REQUIRE_OPENXML_SDK").as_deref(),
            Ok("1"),
            "Open XML SDK proof is required on this runner"
        );
        return;
    }
    let output = Command::new(dotnet.expect("checked dotnet path"))
        .arg(&validator)
        .arg(document)
        .output()
        .expect("run Open XML SDK validator");
    assert_success(&output, "Open XML SDK DOCX validation");
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 errors"));
}

fn assert_libreoffice_heading_render(temp: &Path, document: &Path) {
    let required = ["/usr/bin/soffice", "/usr/bin/pdfinfo", "/usr/bin/pdftotext"];
    if required.iter().any(|tool| !Path::new(tool).is_file()) {
        println!("SKIP LibreOffice DOCX build render: missing soffice, pdfinfo, or pdftotext");
        return;
    }
    let rendered = temp.join("rendered");
    fs::create_dir_all(&rendered).expect("create render directory");
    let profile = temp.join("lo-profile");
    let convert = Command::new("/usr/bin/soffice")
        .arg("--headless")
        .arg(format!(
            "-env:UserInstallation=file://{}",
            profile.display()
        ))
        .args(["--convert-to", "pdf", "--outdir"])
        .arg(&rendered)
        .arg(document)
        .output()
        .expect("run LibreOffice DOCX render");
    assert_success(&convert, "LibreOffice DOCX render");
    let pdf = rendered.join("quarterly-report.pdf");
    assert!(
        pdf.is_file(),
        "LibreOffice did not produce {}",
        pdf.display()
    );
    let info = Command::new("/usr/bin/pdfinfo")
        .arg(&pdf)
        .output()
        .expect("run pdfinfo");
    assert_success(&info, "pdfinfo");
    let info = String::from_utf8_lossy(&info.stdout);
    let pages = info
        .lines()
        .find_map(|line| line.strip_prefix("Pages:").map(str::trim))
        .and_then(|value| value.parse::<usize>().ok())
        .expect("PDF page count");
    assert!(
        pages >= 2,
        "page-break recipe must render at least two pages: {info}"
    );

    let text_file = rendered.join("quarterly-report.txt");
    let text = Command::new("/usr/bin/pdftotext")
        .arg("-layout")
        .arg(&pdf)
        .arg(&text_file)
        .output()
        .expect("extract rendered PDF text");
    assert_success(&text, "pdftotext");
    let text = fs::read_to_string(text_file).expect("read rendered text");
    let title = text.find("Q3 quarterly report").expect("rendered title");
    let highlights = text.find("Highlights").expect("rendered heading");
    let outlook = text.rfind("Outlook").expect("rendered outlook heading");
    assert!(
        title < highlights && highlights < outlook,
        "rendered reading order drifted: {text}"
    );
    for expected in ["Regional metrics", "Product adoption accelerated", "Page"] {
        assert!(
            text.contains(expected),
            "rendered report missing {expected:?}: {text}"
        );
    }

    let bbox_file = rendered.join("quarterly-report-bbox.html");
    let bbox = Command::new("/usr/bin/pdftotext")
        .arg("-bbox")
        .arg(&pdf)
        .arg(&bbox_file)
        .output()
        .expect("extract rendered PDF bounding boxes");
    assert_success(&bbox, "pdftotext -bbox");
    let bbox = fs::read_to_string(bbox_file).expect("read PDF bounding boxes");
    let heading_height = rendered_word_height(&bbox, "Highlights");
    let body_height = rendered_word_height(&bbox, "Revenue");
    assert!(
        heading_height > body_height + 1.0,
        "Heading1 must render materially larger than body text: heading={heading_height}, body={body_height}"
    );
}

fn rendered_word_height(bbox: &str, word: &str) -> f64 {
    let expression = Regex::new(&format!(
        r#"<word xMin=\"[^\"]+\" yMin=\"([^\"]+)\" xMax=\"[^\"]+\" yMax=\"([^\"]+)\">{}</word>"#,
        regex::escape(word)
    ))
    .expect("valid bbox expression");
    let captures = expression
        .captures(bbox)
        .unwrap_or_else(|| panic!("rendered word {word:?} missing from bbox output"));
    let y_min = captures[1].parse::<f64>().expect("bbox yMin");
    let y_max = captures[2].parse::<f64>().expect("bbox yMax");
    y_max - y_min
}

fn zip_text(package: &Path, part: &str) -> String {
    let mut archive = ZipArchive::new(File::open(package).expect("open DOCX")).expect("open ZIP");
    let mut text = String::new();
    archive
        .by_name(part)
        .unwrap_or_else(|error| panic!("missing {part}: {error}"))
        .read_to_string(&mut text)
        .unwrap_or_else(|error| panic!("read {part}: {error}"));
    text
}
