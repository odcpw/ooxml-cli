use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const GOLDEN_CASES: &[(&str, &str, &[&str], &str)] = &[
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
        &["--sheet", "Sparse"],
        "xlsx",
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

#[test]
fn family_outlines_match_nine_deterministic_fixture_goldens() {
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
        assert_eq!(document["depth"], 3, "{name}");
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
    assert!(
        elapsed < Duration::from_secs(2),
        "largest fixture outline took {elapsed:?}"
    );
}
