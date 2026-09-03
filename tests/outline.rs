use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const GOLDEN_CASES: &[(&str, &str, &[&str], &str)] = &[
    (
        "pptx-python-pptx",
        "testdata/pptx/producers/python-pptx/presentation.pptx",
        &["--depth", "1", "--slide", "1", "--text-preview", "12"],
        "pptx",
    ),
    (
        "pptx-libreoffice",
        "testdata/pptx/producers/libreoffice/presentation.pptx",
        &["--depth", "2", "--slide", "2", "--text-preview", "16"],
        "pptx",
    ),
    (
        "pptx-multi-layout",
        "testdata/pptx/multi-layout/presentation.pptx",
        &["--depth", "3", "--slide", "3", "--text-preview", "40"],
        "pptx",
    ),
    (
        "pptx-chart-simple",
        "testdata/pptx/chart-simple/presentation.pptx",
        &[],
        "pptx",
    ),
    (
        "pptx-table-simple",
        "testdata/pptx/table-simple/presentation.pptx",
        &["--slide", "2", "--text-preview", "32"],
        "pptx",
    ),
    (
        "pptx-notes-media",
        "testdata/pptx/slide-assembly-notes-media/presentation.pptx",
        &["--slide", "3", "--text-preview", "24"],
        "pptx",
    ),
    (
        "xlsx-chart-workbook",
        "testdata/xlsx/chart-workbook/workbook.xlsx",
        &[],
        "xlsx",
    ),
    (
        "xlsx-used-range",
        "testdata/xlsx/used-range/workbook.xlsx",
        &["--depth", "2", "--sheet", "Sparse", "--text-preview", "24"],
        "xlsx",
    ),
    (
        "xlsx-minimal",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &["--depth", "0", "--text-preview", "0"],
        "xlsx",
    ),
    (
        "xlsx-libreoffice-chart",
        "testdata/xlsx/libreoffice-chart-workbook/workbook.xlsx",
        &["--depth", "1", "--sheet", "Data"],
        "xlsx",
    ),
    (
        "xlsx-pivot",
        "testdata/xlsx/invalid/pivot-table-parts.xlsx",
        &["--depth", "3", "--sheet", "1"],
        "xlsx",
    ),
    (
        "xlsx-types-formulas",
        "testdata/xlsx/types-and-formulas/workbook.xlsx",
        &["--depth", "2", "--sheet", "Types", "--text-preview", "12"],
        "xlsx",
    ),
    (
        "xlsx-table",
        "testdata/xlsx/outline-table/workbook.xlsx",
        &["--depth", "3", "--sheet", "Data", "--text-preview", "20"],
        "xlsx",
    ),
    (
        "xlsx-names",
        "testdata/xlsx/outline-names/workbook.xlsx",
        &["--depth", "3", "--sheet", "Data"],
        "xlsx",
    ),
    (
        "docx-minimal",
        "testdata/docx/minimal/document.docx",
        &["--depth", "0", "--text-preview", "0"],
        "docx",
    ),
    (
        "docx-comments",
        "testdata/docx/with-comments/document.docx",
        &["--depth", "2", "--section", "1", "--text-preview", "16"],
        "docx",
    ),
    (
        "docx-fields",
        "testdata/docx/with-fields/document.docx",
        &[],
        "docx",
    ),
    (
        "docx-image",
        "testdata/docx/with-image/document.docx",
        &["--section", "1", "--text-preview", "20"],
        "docx",
    ),
    (
        "docx-headers",
        "testdata/docx/headers/document.docx",
        &[],
        "docx",
    ),
    (
        "docx-table",
        "testdata/docx/table/document.docx",
        &[],
        "docx",
    ),
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .output()
        .expect("run ooxml")
}

fn run_outline(file: &str, extra: &[&str]) -> Output {
    let mut args = vec!["--json", "outline", file];
    args.extend_from_slice(extra);
    run(&args)
}

fn successful_json(output: &Output, context: &str) -> Value {
    assert!(
        output.status.success(),
        "{context}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{context}: unexpected stderr");
    serde_json::from_slice(&output.stdout).expect("outline JSON")
}

fn assert_or_update_golden(name: &str, actual: &[u8]) {
    let path = Path::new("testdata/golden/outline").join(format!("{name}.json"));
    if std::env::var_os("UPDATE_GOLDENS").is_some() {
        fs::create_dir_all(path.parent().expect("golden parent")).expect("create golden directory");
        fs::write(&path, actual).expect("write outline golden");
        return;
    }
    let expected = fs::read(&path).unwrap_or_else(|err| {
        panic!(
            "missing outline golden {}: {err}; run UPDATE_GOLDENS=1 cargo test --test outline",
            path.display()
        )
    });
    assert_eq!(actual, expected, "outline golden {}", path.display());
}

fn nested_items<'a>(document: &'a Value, collection: &str, nested: &str) -> Vec<&'a Value> {
    document[collection]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|item| item[nested].as_array().into_iter().flatten())
        .collect()
}

fn golden_document<'a>(documents: &'a [(&str, Value)], name: &str) -> &'a Value {
    &documents
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .expect("golden document")
        .1
}

fn expected_usize_flag(args: &[&str], flag: &str, default: usize) -> usize {
    args.windows(2)
        .find(|pair| pair[0] == flag)
        .map_or(default, |pair| pair[1].parse().expect("numeric flag"))
}

#[test]
fn json_golden_contracts_are_lf_only_on_every_runner() {
    for directory in ["outline", "check", "design-check"] {
        let path = format!("testdata/golden/{directory}/attribute-contract.json");
        let output = Command::new("git")
            .args(["check-attr", "text", "eol", "--", &path])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("run git check-attr for JSON golden");
        assert!(
            output.status.success(),
            "git check-attr failed for {path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let attributes = String::from_utf8(output.stdout).expect("git check-attr UTF-8 output");
        assert!(
            attributes
                .lines()
                .any(|line| line == format!("{path}: text: set")),
            "{path} must be treated as text: {attributes}"
        );
        assert!(
            attributes
                .lines()
                .any(|line| line == format!("{path}: eol: lf")),
            "{path} must be checked out with LF: {attributes}"
        );
    }

    for (name, _, _, _) in GOLDEN_CASES {
        let path = Path::new("testdata/golden/outline").join(format!("{name}.json"));
        let bytes = fs::read(&path).expect("read outline golden");
        assert!(
            !bytes.contains(&b'\r'),
            "{} contains CR bytes",
            path.display()
        );
        assert!(bytes.ends_with(b"\n"), "{} must end in LF", path.display());
    }
}

#[test]
fn family_outlines_match_twenty_deterministic_fixture_goldens() {
    for (family, expected) in [("pptx", 6), ("xlsx", 8), ("docx", 6)] {
        let count = GOLDEN_CASES
            .iter()
            .filter(|(_, _, _, candidate)| *candidate == family)
            .count();
        assert_eq!(count, expected, "{family} fixture count");
        assert!(count >= 6, "{family} needs at least six fixture goldens");
    }

    let mut documents = Vec::new();
    for (name, file, extra, family) in GOLDEN_CASES {
        let first = run_outline(file, extra);
        let second = run_outline(file, extra);
        assert!(
            first.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&first.stderr)
        );
        assert_eq!(
            first.stdout, second.stdout,
            "{name}: nondeterministic stdout"
        );
        assert_eq!(
            first.stderr, second.stderr,
            "{name}: nondeterministic stderr"
        );
        assert_or_update_golden(name, &first.stdout);
        let document = successful_json(&first, name);
        assert_eq!(document["schemaVersion"], 1, "{name}");
        assert_eq!(document["type"], *family, "{name}");
        assert_eq!(
            document["depth"],
            expected_usize_flag(extra, "--depth", 3),
            "{name}"
        );
        assert_eq!(
            document["textPreviewChars"],
            expected_usize_flag(extra, "--text-preview", 80),
            "{name}"
        );
        assert_eq!(document["file"], *file, "{name}");
        assert!(document["summary"].is_object(), "{name}");
        documents.push((*name, document));
    }

    let chart_deck = golden_document(&documents, "pptx-chart-simple");
    assert!(chart_deck["slideSize"]["emu"]["width"].is_number());
    assert!(chart_deck["slideSize"]["inches"]["width"].is_number());
    assert!(!chart_deck["theme"].is_null());
    assert!(
        !chart_deck["masters"]
            .as_array()
            .expect("masters")
            .is_empty()
    );
    assert!(
        !chart_deck["layouts"]
            .as_array()
            .expect("layouts")
            .is_empty()
    );
    assert!(
        nested_items(chart_deck, "slides", "charts")
            .iter()
            .any(|chart| chart["selector"].is_string())
    );
    assert!(
        nested_items(chart_deck, "slides", "shapes")
            .iter()
            .any(|shape| {
                shape["selector"].is_string()
                    && shape["handle"].is_string()
                    && shape["kind"].is_string()
                    && shape["bounds"]["x"].is_number()
                    && shape["bounds"]["inches"]["x"].is_number()
            })
    );
    assert!(
        !nested_items(
            golden_document(&documents, "pptx-table-simple"),
            "slides",
            "tables"
        )
        .is_empty()
    );
    let notes_media = golden_document(&documents, "pptx-notes-media");
    assert_eq!(notes_media["slides"][0]["notes"], true);
    assert!(!nested_items(notes_media, "slides", "images").is_empty());

    let chart_book = golden_document(&documents, "xlsx-chart-workbook");
    assert!(chart_book["sheets"][0]["usedRange"].is_object());
    assert!(chart_book["sheets"][0].get("freeze").is_some());
    assert!(chart_book["sheets"][0]["validationCount"].is_number());
    assert!(chart_book["sheets"][0]["conditionalFormatCount"].is_number());
    assert!(chart_book["sheets"][0]["commentCount"].is_number());
    assert!(!nested_items(chart_book, "sheets", "charts").is_empty());
    assert_eq!(
        golden_document(&documents, "xlsx-used-range")["scope"]["sheet"],
        "Sparse"
    );
    assert_eq!(
        golden_document(&documents, "xlsx-pivot")["sheets"][0]["pivots"][0]["primarySelector"],
        "pivot:1"
    );
    assert!(
        !nested_items(
            golden_document(&documents, "xlsx-table"),
            "sheets",
            "tables"
        )
        .is_empty()
    );
    assert_eq!(
        golden_document(&documents, "xlsx-names")["names"][0]["name"],
        "DataRange"
    );

    let fields = golden_document(&documents, "docx-fields");
    assert!(
        fields["documentHash"]
            .as_str()
            .is_some_and(|hash| hash.starts_with("sha256:"))
    );
    assert!(
        fields["blocks"]
            .as_array()
            .expect("blocks")
            .iter()
            .all(|block| {
                block["kind"].is_string()
                    && block["contentHash"]
                        .as_str()
                        .is_some_and(|hash| hash.starts_with("sha256:"))
            })
    );
    assert!(!fields["fields"].as_array().expect("fields").is_empty());
    assert!(
        !golden_document(&documents, "docx-image")["images"]
            .as_array()
            .expect("images")
            .is_empty()
    );
    assert!(
        !golden_document(&documents, "docx-headers")["headers"]
            .as_array()
            .expect("headers")
            .is_empty()
    );
    assert!(
        !golden_document(&documents, "docx-table")["tables"]
            .as_array()
            .expect("tables")
            .is_empty()
    );
    assert!(fields["sections"][0]["pageSetup"].is_object());
    assert!(fields["coreProperties"].is_object());
    assert_eq!(
        golden_document(&documents, "docx-comments")["summary"]["comments"],
        true
    );
}

#[test]
fn generated_xlsx_table_and_name_outline_fixtures_are_reproducible() {
    let temp = std::env::temp_dir().join(format!(
        "ooxml-outline-xlsx-fixtures-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).expect("create fixture reproduction directory");
    let table = temp.join("table.xlsx");
    let names = temp.join("names.xlsx");
    for args in [
        vec![
            "--json",
            "xlsx",
            "tables",
            "create",
            "testdata/xlsx/chart-workbook/workbook.xlsx",
            "--sheet",
            "Data",
            "--range",
            "A1:B4",
            "--table",
            "Sales",
            "--out",
            table.to_str().expect("table path"),
        ],
        vec![
            "--json",
            "xlsx",
            "names",
            "add",
            "testdata/xlsx/chart-workbook/workbook.xlsx",
            "--name",
            "DataRange",
            "--ref",
            "Data!$A$1:$B$4",
            "--out",
            names.to_str().expect("names path"),
        ],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "fixture reproduction failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read(table).expect("read regenerated table workbook"),
        fs::read("testdata/xlsx/outline-table/workbook.xlsx")
            .expect("read committed table workbook")
    );
    assert_eq!(
        fs::read(names).expect("read regenerated names workbook"),
        fs::read("testdata/xlsx/outline-names/workbook.xlsx")
            .expect("read committed names workbook")
    );
    fs::remove_dir_all(temp).expect("remove fixture reproduction directory");
}

#[test]
fn depth_and_family_scope_contracts_are_enforced() {
    let summary = successful_json(
        &run_outline(
            "testdata/pptx/title-content/presentation.pptx",
            &["--depth", "0", "--text-preview", "0"],
        ),
        "depth zero",
    );
    assert_eq!(summary["depth"], 0);
    assert_eq!(summary["textPreviewChars"], 0);
    assert!(summary.get("slides").is_none());
    assert!(summary["slideSize"]["emu"].is_object());

    let slide = successful_json(
        &run_outline(
            "testdata/pptx/chart-simple/presentation.pptx",
            &["--depth", "2", "--slide", "2", "--text-preview", "8"],
        ),
        "slide scope",
    );
    assert_eq!(slide["scope"]["slide"], 2);
    assert_eq!(slide["slides"].as_array().expect("slides").len(), 1);
    assert_eq!(slide["slides"][0]["number"], 2);
    assert!(slide["slides"][0].get("shapes").is_some());
    assert!(slide["slides"][0].get("charts").is_none());
    for shape in slide["slides"][0]["shapes"].as_array().expect("shapes") {
        if let Some(preview) = shape["textPreview"].as_str() {
            assert!(preview.chars().count() <= 8, "{preview:?}");
        }
    }

    let mismatch = run_outline(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &["--slide", "1"],
    );
    assert_eq!(mismatch.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&mismatch.stderr).expect("JSON error");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("--sheet"))
    );
}

#[test]
fn largest_committed_fixture_outline_stays_under_two_seconds() {
    let file = "testdata/pptx/edge-large-deck/presentation.pptx";
    let warm = run_outline(file, &[]);
    assert!(
        warm.status.success(),
        "warm-up: {}",
        String::from_utf8_lossy(&warm.stderr)
    );

    let started = Instant::now();
    let measured = run_outline(file, &[]);
    let elapsed = started.elapsed();
    assert!(
        measured.status.success(),
        "measured: {}",
        String::from_utf8_lossy(&measured.stderr)
    );
    eprintln!("largest committed fixture outline elapsed: {elapsed:?}");
    if std::env::var("OOXML_PERF_BUDGETS").as_deref() == Ok("1") {
        assert!(
            elapsed < Duration::from_secs(2),
            "largest fixture outline took {elapsed:?}"
        );
    }
}
