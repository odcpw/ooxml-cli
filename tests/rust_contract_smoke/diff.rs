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
fn top_level_diff_render_is_an_explicit_rust_gap() {
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "diff",
        "testdata/pptx/title-content/presentation.pptx",
        "testdata/pptx/title-content/presentation.pptx",
        "--render",
    ]);

    assert_eq!(code, 2);
    assert_eq!(stdout, None);
    let error = stderr.expect("render gap error");
    assert_eq!(error["error"]["code"], "invalid_args");
    assert!(
        error["error"]["message"]
            .as_str()
            .expect("message")
            .contains("not yet supported")
    );
}
