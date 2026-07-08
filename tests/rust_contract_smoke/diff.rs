// Top-level `ooxml diff` contract tests.
use super::*;

#[test]
fn top_level_diff_xlsx_matches_rust_baseline_for_cell_value_change() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-diff-xlsx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let candidate = temp_dir.join("candidate.xlsx");
    rewrite_zip_fixture(
        "testdata/xlsx/types-and-formulas/workbook.xlsx",
        &candidate,
        |name, data| {
            let data = if name == "xl/worksheets/sheet1.xml" {
                replace_ascii(
                    data,
                    r#"<c r="B2"><v>1234.5</v></c>"#,
                    r#"<c r="B2"><v>4321</v></c>"#,
                )
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let candidate = candidate.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "diff",
        "testdata/xlsx/types-and-formulas/workbook.xlsx",
        &candidate,
    ]);
}

#[test]
fn top_level_diff_xlsx_aligns_renamed_sheet_by_stable_identity() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-diff-xlsx-rename-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let baseline_path = temp_dir.join("baseline.xlsx");
    let candidate_path = temp_dir.join("candidate.xlsx");
    write_simple_xlsx_with_sheet_xml(
        &baseline_path,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData>
</worksheet>"#,
    );
    rewrite_zip_fixture(
        baseline_path.to_str().expect("baseline path"),
        &candidate_path,
        |name, data| {
            let data = if name == "xl/workbook.xml" {
                replace_ascii(data, r#"name="Sheet1""#, r#"name="Renamed""#)
            } else if name == "xl/worksheets/sheet1.xml" {
                replace_ascii(data, "<v>1</v>", "<v>2</v>")
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let baseline = baseline_path.to_string_lossy().to_string();
    let candidate = candidate_path.to_string_lossy().to_string();

    let (code, stdout, stderr) = run_ooxml(&["--json", "diff", &baseline, &candidate]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let output = stdout.expect("diff stdout");
    let sheets = output["semantic"]["sheets"]
        .as_array()
        .expect("sheet diffs");
    let rename = sheets
        .iter()
        .find(|item| item["change"] == "renamed")
        .expect("renamed sheet diff");
    assert_eq!(rename["sheet"], "Renamed");
    assert_eq!(rename["before"], "Sheet1");
    assert_eq!(rename["after"], "Renamed");
    assert_eq!(
        rename["identity"]["partUriBefore"],
        "xl/worksheets/sheet1.xml"
    );
    assert_eq!(
        rename["identity"]["partUriAfter"],
        "xl/worksheets/sheet1.xml"
    );
    assert!(
        sheets
            .iter()
            .all(|item| item["change"] != "removed" && item["change"] != "added"),
        "rename should not be reported as removed+added: {sheets:?}"
    );
    let cell_diffs = output["semantic"]["cellDiffs"]
        .as_array()
        .expect("cell diffs");
    let value_diff = cell_diffs
        .iter()
        .find(|item| item["cell"] == "A1" && item["property"] == "value")
        .expect("A1 value diff");
    assert_eq!(value_diff["sheet"], "Renamed");
    assert_eq!(value_diff["before"], "1");
    assert_eq!(value_diff["after"], "2");
    assert!(
        output["semantic"]["changedSheets"]
            .as_array()
            .expect("changed sheets")
            .iter()
            .any(|item| item.as_str() == Some("Renamed"))
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn top_level_diff_docx_matches_rust_baseline_for_paragraph_text_change() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-diff-docx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let candidate = temp_dir.join("candidate.docx");
    rewrite_zip_fixture(
        "testdata/docx/mixed-blocks/document.docx",
        &candidate,
        |name, data| {
            let data = if name == "word/document.xml" {
                replace_ascii(data, "<w:t>Tail paragraph</w:t>", "<w:t>Tail edited</w:t>")
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let candidate = candidate.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "diff",
        "testdata/docx/mixed-blocks/document.docx",
        &candidate,
    ]);
}

#[test]
fn top_level_diff_pptx_matches_rust_baseline_for_title_text_change() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-diff-pptx-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let candidate = temp_dir.join("candidate.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/title-content/presentation.pptx",
        &candidate,
        |name, data| {
            let data = if name == "ppt/slides/slide1.xml" {
                replace_ascii(
                    data,
                    "<a:t>Title Content Presentation</a:t>",
                    "<a:t>Renamed Title</a:t>",
                )
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let candidate = candidate.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "diff",
        "testdata/pptx/title-content/presentation.pptx",
        &candidate,
    ]);
}

#[test]
fn pptx_diff_path_matches_rust_baseline_for_title_text_change() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-pptx-diff-path-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let candidate = temp_dir.join("candidate.pptx");
    rewrite_zip_fixture(
        "testdata/pptx/title-content/presentation.pptx",
        &candidate,
        |name, data| {
            let data = if name == "ppt/slides/slide1.xml" {
                replace_ascii(
                    data,
                    "<a:t>Title Content Presentation</a:t>",
                    "<a:t>Renamed Title</a:t>",
                )
            } else {
                data
            };
            Some((name.to_string(), data))
        },
    );
    let candidate = candidate.to_string_lossy().to_string();

    assert_rust_baseline_match(&[
        "--json",
        "pptx",
        "diff",
        "testdata/pptx/title-content/presentation.pptx",
        &candidate,
    ]);
}

#[test]
fn top_level_diff_mismatched_family_error_matches_rust_baseline() {
    assert_rust_baseline_match(&[
        "--json",
        "diff",
        "testdata/xlsx/types-and-formulas/workbook.xlsx",
        "testdata/docx/mixed-blocks/document.docx",
    ]);
}

#[test]
fn top_level_diff_render_reports_unavailable_tools_with_visual_payload() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-diff-render-path-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let (code, stdout, stderr) = run_ooxml_with_path(
        &[
            "--json",
            "diff",
            "testdata/pptx/title-content/presentation.pptx",
            "testdata/pptx/title-content/presentation.pptx",
            "--render",
        ],
        temp_dir.to_str().expect("temp path"),
    );

    assert_eq!(code, 9);
    assert_eq!(stderr, None);
    let output = stdout.expect("render diff stdout");
    assert_eq!(output["visual"]["enabled"], true);
    assert_eq!(output["visual"]["status"], "unavailable");
    assert_eq!(output["visual"]["threshold"], 0.01);
}

#[test]
fn pptx_diff_render_mock_mode_emits_deterministic_visual_artifacts() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-diff-render-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let out_dir = temp_dir.join("visual");
    let out_dir_str = out_dir.to_str().expect("out dir");

    let (code, stdout, stderr) = run_ooxml_with_env(
        &[
            "--json",
            "pptx",
            "diff",
            "testdata/pptx/title-content/presentation.pptx",
            "testdata/pptx/title-content/presentation.pptx",
            "--render",
            "--out",
            out_dir_str,
        ],
        &[("OOXML_RUST_MOCK_RENDER", "1")],
    );

    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let output = stdout.expect("render diff stdout");
    assert_eq!(output["visual"]["enabled"], true);
    assert_eq!(output["visual"]["status"], "ok");
    assert_eq!(output["visual"]["pass"], true);
    let slides = output["visual"]["slides"].as_array().expect("slides");
    assert_eq!(slides.len(), 2);
    for (index, slide) in slides.iter().enumerate() {
        assert_eq!(slide["slide"], index + 1);
        assert_eq!(slide["difference"], 0.0);
        assert_eq!(slide["pass"], true);
        let diff_image = slide["diffImage"].as_str().expect("diff image");
        assert!(
            Path::new(diff_image).exists(),
            "diff image should exist: {diff_image}"
        );
    }
    assert!(out_dir.join("baseline").join("slide-1.png").exists());
    assert!(out_dir.join("candidate").join("slide-1.png").exists());
    assert!(out_dir.join("diff").join("slide-1-diff.png").exists());
}

fn run_ooxml_with_path(args: &[&str], path: &str) -> (i32, Option<Value>, Option<Value>) {
    let output = Command::new(env!("CARGO_BIN_EXE_ooxml"))
        .args(args)
        .env("PATH", path)
        .env("Path", path)
        .output()
        .expect("run Rust ooxml with PATH");
    let code = output.status.code().unwrap_or(-1);
    let stdout = parse_json(&output.stdout);
    let stderr = parse_json(&output.stderr);
    (code, stdout, stderr)
}

#[test]
fn top_level_diff_render_strict_reports_render_failed_exit_with_visual_payload() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-diff-render-strict-path-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let (code, stdout, stderr) = run_ooxml_with_path(
        &[
            "--json",
            "--strict",
            "diff",
            "testdata/pptx/title-content/presentation.pptx",
            "testdata/pptx/title-content/presentation.pptx",
            "--render",
        ],
        temp_dir.to_str().expect("temp path"),
    );

    assert_eq!(code, 7);
    assert_eq!(stderr, None);
    let output = stdout.expect("render diff stdout");
    assert_eq!(output["visual"]["enabled"], true);
    assert_eq!(output["visual"]["status"], "unavailable");
}

#[test]
fn pptx_diff_render_unavailable_matches_top_level_shape_without_family_envelope() {
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-diff-render-path-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let (code, stdout, stderr) = run_ooxml_with_path(
        &[
            "--json",
            "pptx",
            "diff",
            "testdata/pptx/title-content/presentation.pptx",
            "testdata/pptx/title-content/presentation.pptx",
            "--render",
        ],
        temp_dir.to_str().expect("temp path"),
    );

    assert_eq!(code, 9);
    assert_eq!(stderr, None);
    let output = stdout.expect("render diff stdout");
    assert!(output.get("schemaVersion").is_none());
    assert!(output.get("type").is_none());
    assert_eq!(output["visual"]["enabled"], true);
    assert_eq!(output["visual"]["status"], "unavailable");
}
