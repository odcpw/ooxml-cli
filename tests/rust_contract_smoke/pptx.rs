// PPTX frozen mutation/render/verify contract tests live here while shared
// baseline and process helpers remain in the parent integration test crate.
use super::*;

#[test]
fn frozen_pptx_mutation_and_validate_match_go_baseline() {
    let baseline = baseline();
    let temp_dir = std::env::temp_dir().join(format!("ooxml-rust-contract-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir");
    let edited = temp_dir.join("edited.pptx");
    let render_dir = temp_dir.join("rendered");
    let edited_str = edited.to_str().expect("temp path");
    let render_dir_str = render_dir.to_str().expect("render path");

    let edit_args = [
        "--json",
        "pptx",
        "replace",
        "text",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "1",
        "--target",
        "title",
        "--text",
        "Rust Port Contract",
        "--out",
        edited_str,
    ];
    let (edit_code, edit_stdout, edit_stderr) = run_ooxml(&edit_args);
    assert_eq!(edit_code, 0);
    assert_eq!(edit_stderr, None);
    let edit_expected = baseline["mutation"]["edit"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            edit_stdout.expect("edit stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        edit_expected
    );
    assert!(edited.exists());

    let validate_args = ["--json", "--strict", "validate", edited_str];
    let (validate_code, validate_stdout, validate_stderr) = run_ooxml(&validate_args);
    assert_eq!(validate_code, 0);
    assert_eq!(validate_stderr, None);
    let validate_expected = baseline["mutation"]["validate"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            validate_stdout.expect("validate stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        validate_expected
    );

    let render_args = [
        "pptx",
        "render",
        edited_str,
        "--out",
        render_dir_str,
        "--slides",
        "1",
        "--format",
        "json",
    ];
    let (render_code, render_stdout, render_stderr) =
        run_ooxml_with_env(&render_args, &[("OOXML_RUST_MOCK_RENDER", "1")]);
    assert_eq!(render_code, 0);
    assert_eq!(render_stderr, None);
    let render_expected = baseline["mutation"]["render"]["stdoutJson"].clone();
    assert_eq!(
        scrub_paths(
            render_stdout.expect("render stdout"),
            &[
                (edited_str, "[EDITED_PPTX]"),
                (render_dir_str, "[RENDER_DIR]")
            ]
        ),
        render_expected
    );

    let verify_args = [
        "--format",
        "json",
        "verify",
        edited_str,
        "--baseline",
        "testdata/pptx/minimal-title/presentation.pptx",
    ];
    let (verify_code, verify_stdout, verify_stderr) = run_ooxml(&verify_args);
    assert_eq!(verify_code, 0);
    assert_eq!(verify_stderr, None);
    let verify_expected = baseline["mutation"]["verify"]["stdoutJson"].clone();
    assert_eq!(
        scrub_path(
            verify_stdout.expect("verify stdout"),
            edited_str,
            "[EDITED_PPTX]"
        ),
        verify_expected
    );
}
