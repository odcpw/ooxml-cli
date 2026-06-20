// VBA package-level parity tests live in a child module so the opaque macro
// wiring surface can grow without bloating the parent harness.
use super::*;

#[test]
fn vba_opaque_attach_extract_remove_matches_go_oracle() {
    let temp_dir =
        std::env::temp_dir().join(format!("ooxml-rust-vba-opaque-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let bin_path = temp_dir.join("vbaProject.bin");
    let payload = b"not-real-vba-but-nonempty";
    fs::write(&bin_path, payload).expect("write vbaProject.bin");
    let go_in_path = temp_dir.join("go-input.xlsx");
    let rust_in_path = temp_dir.join("rust-input.xlsx");
    let go_xlsm_path = temp_dir.join("go-output.xlsm");
    let rust_xlsm_path = temp_dir.join("rust-output.xlsm");
    let go_extract_path = temp_dir.join("go-extract.bin");
    let rust_extract_path = temp_dir.join("rust-extract.bin");
    let go_removed_path = temp_dir.join("go-removed.xlsx");
    let rust_removed_path = temp_dir.join("rust-removed.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");

    let bin = bin_path.to_string_lossy().to_string();
    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_xlsm = go_xlsm_path.to_string_lossy().to_string();
    let rust_xlsm = rust_xlsm_path.to_string_lossy().to_string();
    let go_extract = go_extract_path.to_string_lossy().to_string();
    let rust_extract = rust_extract_path.to_string_lossy().to_string();
    let go_removed = go_removed_path.to_string_lossy().to_string();
    let rust_removed = rust_removed_path.to_string_lossy().to_string();

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "inspect", &go_in]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "vba", "inspect", &rust_in]);
    assert_eq!(rust_code, go_code, "initial inspect exit");
    assert_eq!(rust_stderr, go_stderr, "initial inspect stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust initial inspect"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go initial inspect"), &go_in, "[IN]"),
        "initial inspect stdout"
    );

    let go_attach_args = [
        "--json", "vba", "attach", &go_in, "--bin", &bin, "--out", &go_xlsm,
    ];
    let rust_attach_args = [
        "--json", "vba", "attach", &rust_in, "--bin", &bin, "--out", &rust_xlsm,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_attach_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_attach_args);
    assert_eq!(rust_code, go_code, "attach exit");
    assert_eq!(rust_stderr, go_stderr, "attach stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust attach stdout"),
            &[(&rust_in, "[IN]"), (&rust_xlsm, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go attach stdout"),
            &[(&go_in, "[IN]"), (&go_xlsm, "[OUT]")]
        ),
        "attach stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "inspect", &go_xlsm]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "inspect", &rust_xlsm]);
    assert_eq!(rust_code, go_code, "attached inspect exit");
    assert_eq!(rust_stderr, go_stderr, "attached inspect stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust attached inspect"),
            &rust_xlsm,
            "[OUT]"
        ),
        scrub_path(go_stdout.expect("go attached inspect"), &go_xlsm, "[OUT]"),
        "attached inspect stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "extract-bin",
        &go_xlsm,
        "--out",
        &go_extract,
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "extract-bin",
        &rust_xlsm,
        "--out",
        &rust_extract,
    ]);
    assert_eq!(rust_code, go_code, "extract-bin exit");
    assert_eq!(rust_stderr, go_stderr, "extract-bin stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust extract-bin stdout"),
            &[(&rust_xlsm, "[OUT]"), (&rust_extract, "[BIN]")]
        ),
        scrub_paths(
            go_stdout.expect("go extract-bin stdout"),
            &[(&go_xlsm, "[OUT]"), (&go_extract, "[BIN]")]
        ),
        "extract-bin stdout"
    );
    assert_eq!(
        fs::read(&rust_extract_path).expect("rust extracted bytes"),
        fs::read(&go_extract_path).expect("go extracted bytes"),
        "extracted vbaProject.bin bytes"
    );

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "vba", "remove", &go_xlsm, "--out", &go_removed]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "remove",
        &rust_xlsm,
        "--out",
        &rust_removed,
    ]);
    assert_eq!(rust_code, go_code, "remove exit");
    assert_eq!(rust_stderr, go_stderr, "remove stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust remove stdout"),
            &[(&rust_xlsm, "[XLSM]"), (&rust_removed, "[REMOVED]")]
        ),
        scrub_paths(
            go_stdout.expect("go remove stdout"),
            &[(&go_xlsm, "[XLSM]"), (&go_removed, "[REMOVED]")]
        ),
        "remove stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "inspect", &go_removed]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "inspect", &rust_removed]);
    assert_eq!(rust_code, go_code, "removed inspect exit");
    assert_eq!(rust_stderr, go_stderr, "removed inspect stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust removed inspect"),
            &rust_removed,
            "[REMOVED]"
        ),
        scrub_path(
            go_stdout.expect("go removed inspect"),
            &go_removed,
            "[REMOVED]"
        ),
        "removed inspect stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "attach",
        &go_in,
        "--bin",
        &bin,
        "--dry-run",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "attach",
        &rust_in,
        "--bin",
        &bin,
        "--dry-run",
    ]);
    assert_eq!(rust_code, go_code, "attach dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "attach dry-run stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust attach dry-run"), &rust_in, "[IN]"),
        scrub_path(go_stdout.expect("go attach dry-run"), &go_in, "[IN]"),
        "attach dry-run stdout"
    );

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "vba", "remove", &go_xlsm, "--dry-run"]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "remove", &rust_xlsm, "--dry-run"]);
    assert_eq!(rust_code, go_code, "remove dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "remove dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust remove dry-run"),
            &rust_xlsm,
            "[XLSM]"
        ),
        scrub_path(go_stdout.expect("go remove dry-run"), &go_xlsm, "[XLSM]"),
        "remove dry-run stdout"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}
