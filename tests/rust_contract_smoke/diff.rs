// Top-level `ooxml diff` contract tests.
use super::*;

#[test]
fn top_level_diff_xlsx_matches_go_oracle_for_cell_value_change() {
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

    assert_go_rust_match(&[
        "--json",
        "diff",
        "testdata/xlsx/types-and-formulas/workbook.xlsx",
        &candidate,
    ]);
}

#[test]
fn top_level_diff_docx_matches_go_oracle_for_paragraph_text_change() {
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

    assert_go_rust_match(&[
        "--json",
        "diff",
        "testdata/docx/mixed-blocks/document.docx",
        &candidate,
    ]);
}

#[test]
fn top_level_diff_pptx_matches_go_oracle_for_title_text_change() {
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

    assert_go_rust_match(&[
        "--json",
        "diff",
        "testdata/pptx/title-content/presentation.pptx",
        &candidate,
    ]);
}

#[test]
fn pptx_diff_path_matches_go_oracle_for_title_text_change() {
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

    assert_go_rust_match(&[
        "--json",
        "pptx",
        "diff",
        "testdata/pptx/title-content/presentation.pptx",
        &candidate,
    ]);
}

#[test]
fn top_level_diff_mismatched_family_error_matches_go_oracle() {
    assert_go_rust_match(&[
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
