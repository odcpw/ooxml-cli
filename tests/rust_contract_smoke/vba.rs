// VBA package-level parity tests live in a child module so the opaque macro
// wiring surface can grow without bloating the parent harness.
use super::*;
use std::collections::BTreeMap;

#[test]
fn vba_create_source_workflow_matches_go_oracle_without_office_com() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-vba-create-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let source_path = temp_dir.join("Module,One.bas");
    let helper_path = temp_dir.join("fake-windows-office-vba-create.ps1");
    fs::write(
        &source_path,
        "Attribute VB_Name = \"ModuleOne\"\r\nPublic Sub Main()\r\nEnd Sub\r\n",
    )
    .expect("write VBA source");
    fs::write(&helper_path, "Write-Output \"{}\"\r\n").expect("write fake helper");

    let go_out = temp_dir.join("go-created.xlsm");
    let rust_out = temp_dir.join("rust-created.xlsm");
    let go_bin = temp_dir.join("go-vbaProject.bin");
    let rust_bin = temp_dir.join("rust-vbaProject.bin");
    let source = source_path.to_string_lossy().to_string();
    let helper = helper_path.to_string_lossy().to_string();
    let go_out = go_out.to_string_lossy().to_string();
    let rust_out = rust_out.to_string_lossy().to_string();
    let go_bin = go_bin.to_string_lossy().to_string();
    let rust_bin = rust_bin.to_string_lossy().to_string();

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "create",
        &go_out,
        "--source",
        &source,
        "--extract-bin",
        &go_bin,
        "--office-create-script",
        &helper,
        "--force",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "create",
        &rust_out,
        "--source",
        &source,
        "--extract-bin",
        &rust_bin,
        "--office-create-script",
        &helper,
        "--force",
    ]);
    assert_eq!(rust_code, go_code, "create exit");
    assert_eq!(rust_stderr, go_stderr, "create stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust create stdout"),
            &[(&rust_out, "[OUT]"), (&rust_bin, "[BIN]")]
        ),
        scrub_paths(
            go_stdout.expect("go create stdout"),
            &[(&go_out, "[OUT]"), (&go_bin, "[BIN]")]
        ),
        "create stdout"
    );

    let bad_xlsx = temp_dir.join("bad.xlsx").to_string_lossy().to_string();
    let bad_pptm = temp_dir.join("bad.pptm").to_string_lossy().to_string();
    let missing_source_out = temp_dir
        .join("missing-source.xlsm")
        .to_string_lossy()
        .to_string();
    let missing_file_out = temp_dir
        .join("missing-file.xlsm")
        .to_string_lossy()
        .to_string();
    let missing_source = temp_dir.join("Missing.bas").to_string_lossy().to_string();
    let missing_helper_out = temp_dir
        .join("missing-helper.xlsm")
        .to_string_lossy()
        .to_string();
    let missing_helper = temp_dir
        .join("missing-helper.ps1")
        .to_string_lossy()
        .to_string();

    let cases = [
        vec![
            "--json",
            "vba",
            "create",
            &bad_xlsx,
            "--source",
            &source,
            "--office-create-script",
            &helper,
        ],
        vec![
            "--json",
            "vba",
            "create",
            &bad_pptm,
            "--family",
            "xlsx",
            "--source",
            &source,
            "--office-create-script",
            &helper,
        ],
        vec![
            "--json",
            "vba",
            "create",
            &missing_source_out,
            "--office-create-script",
            &helper,
        ],
        vec![
            "--json",
            "vba",
            "create",
            &missing_file_out,
            "--source",
            &missing_source,
            "--office-create-script",
            &helper,
        ],
        vec![
            "--json",
            "vba",
            "create",
            &missing_helper_out,
            "--source",
            &source,
            "--office-create-script",
            &missing_helper,
        ],
    ];
    for args in cases {
        assert_go_rust_match(&args);
    }

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn vba_office_check_macro_free_package_matches_go_oracle() {
    assert_go_rust_match(&[
        "--json",
        "vba",
        "office-check",
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
    ]);
}

#[test]
fn vba_source_readback_inspect_list_extract_matches_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-vba-source-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let bin_path = temp_dir.join("vbaProject.bin");
    fs::write(&bin_path, synthetic_vba_project_bin()).expect("write synthetic vbaProject.bin");
    let bin = bin_path.to_string_lossy().to_string();

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "vba", "inspect-bin", &bin, "--family", "xlsx"]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "inspect-bin", &bin, "--family", "xlsx"]);
    assert_eq!(rust_code, go_code, "inspect-bin exit");
    assert_eq!(rust_stderr, go_stderr, "inspect-bin stderr");
    assert_eq!(rust_stdout, go_stdout, "inspect-bin stdout");

    let go_in_path = temp_dir.join("go-input.xlsx");
    let rust_in_path = temp_dir.join("rust-input.xlsx");
    let go_xlsm_path = temp_dir.join("go-output.xlsm");
    let rust_xlsm_path = temp_dir.join("rust-output.xlsm");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &go_in_path).expect("go input");
    fs::copy(
        "testdata/xlsx/minimal-workbook/workbook.xlsx",
        &rust_in_path,
    )
    .expect("rust input");

    let go_in = go_in_path.to_string_lossy().to_string();
    let rust_in = rust_in_path.to_string_lossy().to_string();
    let go_xlsm = go_xlsm_path.to_string_lossy().to_string();
    let rust_xlsm = rust_xlsm_path.to_string_lossy().to_string();

    let (go_code, _, go_stderr) = run_go_ooxml(&[
        "--json", "vba", "attach", &go_in, "--bin", &bin, "--out", &go_xlsm,
    ]);
    let (rust_code, _, rust_stderr) = run_ooxml(&[
        "--json", "vba", "attach", &rust_in, "--bin", &bin, "--out", &rust_xlsm,
    ]);
    assert_eq!(rust_code, go_code, "attach parseable synthetic bin exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "attach parseable synthetic bin stderr"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "list", &go_xlsm]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "vba", "list", &rust_xlsm]);
    assert_eq!(rust_code, go_code, "list exit");
    assert_eq!(rust_stderr, go_stderr, "list stderr");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust list stdout"), &rust_xlsm, "[XLSM]"),
        scrub_path(go_stdout.expect("go list stdout"), &go_xlsm, "[XLSM]"),
        "list stdout"
    );

    let go_extract_dir = temp_dir.join("go-modules");
    let rust_extract_dir = temp_dir.join("rust-modules");
    let go_extract = go_extract_dir.to_string_lossy().to_string();
    let rust_extract = rust_extract_dir.to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "extract",
        &go_xlsm,
        "--out-dir",
        &go_extract,
        "--module",
        "module:Module1",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "extract",
        &rust_xlsm,
        "--out-dir",
        &rust_extract,
        "--module",
        "module:Module1",
    ]);
    assert_eq!(rust_code, go_code, "extract exit");
    assert_eq!(rust_stderr, go_stderr, "extract stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust extract stdout"),
            &[(&rust_xlsm, "[XLSM]"), (&rust_extract, "[DIR]")]
        ),
        scrub_paths(
            go_stdout.expect("go extract stdout"),
            &[(&go_xlsm, "[XLSM]"), (&go_extract, "[DIR]")]
        ),
        "extract stdout"
    );
    assert_eq!(
        fs::read_to_string(rust_extract_dir.join("Module1.bas")).expect("rust Module1"),
        fs::read_to_string(go_extract_dir.join("Module1.bas")).expect("go Module1"),
        "extracted Module1 source"
    );

    let go_missing_extract_dir = temp_dir.join("go-missing-module");
    let rust_missing_extract_dir = temp_dir.join("rust-missing-module");
    let go_missing_extract = go_missing_extract_dir.to_string_lossy().to_string();
    let rust_missing_extract = rust_missing_extract_dir.to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "extract",
        &go_xlsm,
        "--out-dir",
        &go_missing_extract,
        "--module",
        "Modul",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "extract",
        &rust_xlsm,
        "--out-dir",
        &rust_missing_extract,
        "--module",
        "Modul",
    ]);
    assert_eq!(rust_code, go_code, "extract missing module exit");
    assert_eq!(rust_stdout, go_stdout, "extract missing module stdout");
    assert_eq!(
        scrub_path(
            rust_stderr.expect("rust extract missing module stderr"),
            &rust_xlsm,
            "[XLSM]"
        ),
        scrub_path(
            go_stderr.expect("go extract missing module stderr"),
            &go_xlsm,
            "[XLSM]"
        ),
        "extract missing module stderr"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "list", &go_in]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "vba", "list", &rust_in]);
    assert_eq!(rust_code, go_code, "missing macro list exit");
    assert_eq!(rust_stdout, go_stdout, "missing macro list stdout");
    assert_eq!(
        scrub_path(
            rust_stderr.expect("rust missing macro stderr"),
            &rust_in,
            "[IN]"
        ),
        scrub_path(go_stderr.expect("go missing macro stderr"), &go_in, "[IN]"),
        "missing macro list stderr"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn vba_source_module_mutations_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-vba-source-mutate-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let bin_path = temp_dir.join("vbaProject.bin");
    fs::write(&bin_path, synthetic_vba_project_bin()).expect("write synthetic vbaProject.bin");
    let go_in_path = temp_dir.join("go-input.xlsx");
    let rust_in_path = temp_dir.join("rust-input.xlsx");
    let go_xlsm_path = temp_dir.join("go-seed.xlsm");
    let rust_xlsm_path = temp_dir.join("rust-seed.xlsm");
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
    let (go_code, _, go_stderr) = run_go_ooxml(&[
        "--json", "vba", "attach", &go_in, "--bin", &bin, "--out", &go_xlsm,
    ]);
    let (rust_code, _, rust_stderr) = run_ooxml(&[
        "--json", "vba", "attach", &rust_in, "--bin", &bin, "--out", &rust_xlsm,
    ]);
    assert_eq!(rust_code, go_code, "attach seed exit");
    assert_eq!(rust_stderr, go_stderr, "attach seed stderr");

    let add_source_path = temp_dir.join("NewModule.bas");
    fs::write(
        &add_source_path,
        "Attribute VB_Name = \"NewModule\"\r\nPublic Sub Added()\r\nEnd Sub\r\n",
    )
    .expect("write add source");
    let add_source = add_source_path.to_string_lossy().to_string();
    let go_added_path = temp_dir.join("go-added.xlsm");
    let rust_added_path = temp_dir.join("rust-added.xlsm");
    let go_added = go_added_path.to_string_lossy().to_string();
    let rust_added = rust_added_path.to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "add-module",
        &go_xlsm,
        "--source",
        &add_source,
        "--expect-module-count",
        "2",
        "--allow-experimental-vba-source-rewrite",
        "--out",
        &go_added,
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "add-module",
        &rust_xlsm,
        "--source",
        &add_source,
        "--expect-module-count",
        "2",
        "--allow-experimental-vba-source-rewrite",
        "--out",
        &rust_added,
    ]);
    assert_eq!(rust_code, go_code, "add-module exit");
    assert_eq!(rust_stderr, go_stderr, "add-module stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust add-module stdout"),
            &[(&rust_xlsm, "[IN]"), (&rust_added, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go add-module stdout"),
            &[(&go_xlsm, "[IN]"), (&go_added, "[OUT]")]
        ),
        "add-module stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "list", &go_added]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&["--json", "vba", "list", &rust_added]);
    assert_eq!(rust_code, go_code, "list after add exit");
    assert_eq!(rust_stderr, go_stderr, "list after add stderr");
    let go_list_after_add = go_stdout.expect("go list after add");
    let rust_list_after_add = rust_stdout.expect("rust list after add");
    assert_eq!(
        scrub_path(rust_list_after_add.clone(), &rust_added, "[OUT]"),
        scrub_path(go_list_after_add.clone(), &go_added, "[OUT]"),
        "list after add stdout"
    );
    let module1_hash = module_sha256(&rust_list_after_add, "Module1");

    let replace_source_path = temp_dir.join("Module1.bas");
    fs::write(
        &replace_source_path,
        "Attribute VB_Name = \"Module1\"\nPublic Sub HelloWorld()\nDebug.Print \"replaced\"\nEnd Sub",
    )
    .expect("write replacement source");
    let replace_source = replace_source_path.to_string_lossy().to_string();
    let go_replaced_path = temp_dir.join("go-replaced.xlsm");
    let rust_replaced_path = temp_dir.join("rust-replaced.xlsm");
    let go_replaced = go_replaced_path.to_string_lossy().to_string();
    let rust_replaced = rust_replaced_path.to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "replace-module",
        &go_added,
        "--module",
        "module:Module1",
        "--source",
        &replace_source,
        "--expect-sha256",
        &module1_hash,
        "--allow-experimental-vba-source-rewrite",
        "--out",
        &go_replaced,
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "replace-module",
        &rust_added,
        "--module",
        "module:Module1",
        "--source",
        &replace_source,
        "--expect-sha256",
        &module1_hash,
        "--allow-experimental-vba-source-rewrite",
        "--out",
        &rust_replaced,
    ]);
    assert_eq!(rust_code, go_code, "replace-module exit");
    assert_eq!(rust_stderr, go_stderr, "replace-module stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust replace-module stdout"),
            &[(&rust_added, "[IN]"), (&rust_replaced, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go replace-module stdout"),
            &[(&go_added, "[IN]"), (&go_replaced, "[OUT]")]
        ),
        "replace-module stdout"
    );

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&["--json", "vba", "list", &go_replaced]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "vba", "list", &rust_replaced]);
    assert_eq!(rust_code, go_code, "list after replace exit");
    assert_eq!(rust_stderr, go_stderr, "list after replace stderr");
    let go_list_after_replace = go_stdout.expect("go list after replace");
    let rust_list_after_replace = rust_stdout.expect("rust list after replace");
    assert_eq!(
        scrub_path(rust_list_after_replace.clone(), &rust_replaced, "[OUT]"),
        scrub_path(go_list_after_replace.clone(), &go_replaced, "[OUT]"),
        "list after replace stdout"
    );
    let class_hash = module_sha256(&rust_list_after_replace, "Class1");

    let go_removed_module_path = temp_dir.join("go-removed-module.xlsm");
    let rust_removed_module_path = temp_dir.join("rust-removed-module.xlsm");
    let go_removed_module = go_removed_module_path.to_string_lossy().to_string();
    let rust_removed_module = rust_removed_module_path.to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "remove-module",
        &go_replaced,
        "--module",
        "module:Class1",
        "--expect-sha256",
        &class_hash,
        "--allow-experimental-vba-source-rewrite",
        "--out",
        &go_removed_module,
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "remove-module",
        &rust_replaced,
        "--module",
        "module:Class1",
        "--expect-sha256",
        &class_hash,
        "--allow-experimental-vba-source-rewrite",
        "--out",
        &rust_removed_module,
    ]);
    assert_eq!(rust_code, go_code, "remove-module exit");
    assert_eq!(rust_stderr, go_stderr, "remove-module stderr");
    assert_eq!(
        scrub_paths(
            rust_stdout.expect("rust remove-module stdout"),
            &[(&rust_replaced, "[IN]"), (&rust_removed_module, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go remove-module stdout"),
            &[(&go_replaced, "[IN]"), (&go_removed_module, "[OUT]")]
        ),
        "remove-module stdout"
    );

    let dry_source_path = temp_dir.join("DryRunModule.bas");
    fs::write(
        &dry_source_path,
        "Attribute VB_Name = \"DryRunModule\"\r\nPublic Sub DryRun()\r\nEnd Sub\r\n",
    )
    .expect("write dry-run source");
    let dry_source = dry_source_path.to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "add-module",
        &go_removed_module,
        "--source",
        &dry_source,
        "--allow-experimental-vba-source-rewrite",
        "--dry-run",
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "add-module",
        &rust_removed_module,
        "--source",
        &dry_source,
        "--allow-experimental-vba-source-rewrite",
        "--dry-run",
    ]);
    assert_eq!(rust_code, go_code, "add-module dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "add-module dry-run stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust add-module dry-run"),
            &rust_removed_module,
            "[IN]"
        ),
        scrub_path(
            go_stdout.expect("go add-module dry-run"),
            &go_removed_module,
            "[IN]"
        ),
        "add-module dry-run stdout"
    );

    for (label, go_args, rust_args, scrub_go, scrub_rust) in [
        (
            "add-module count mismatch",
            vec![
                "--json",
                "vba",
                "add-module",
                &go_removed_module,
                "--source",
                &dry_source,
                "--expect-module-count",
                "99",
                "--dry-run",
            ],
            vec![
                "--json",
                "vba",
                "add-module",
                &rust_removed_module,
                "--source",
                &dry_source,
                "--expect-module-count",
                "99",
                "--dry-run",
            ],
            go_removed_module.as_str(),
            rust_removed_module.as_str(),
        ),
        (
            "replace-module hash mismatch",
            vec![
                "--json",
                "vba",
                "replace-module",
                &go_removed_module,
                "--module",
                "module:Module1",
                "--source",
                &replace_source,
                "--expect-sha256",
                "0000",
                "--dry-run",
            ],
            vec![
                "--json",
                "vba",
                "replace-module",
                &rust_removed_module,
                "--module",
                "module:Module1",
                "--source",
                &replace_source,
                "--expect-sha256",
                "0000",
                "--dry-run",
            ],
            go_removed_module.as_str(),
            rust_removed_module.as_str(),
        ),
        (
            "replace-module no macro",
            vec![
                "--json",
                "vba",
                "replace-module",
                &go_in,
                "--module",
                "module:Module1",
                "--source",
                &replace_source,
                "--dry-run",
            ],
            vec![
                "--json",
                "vba",
                "replace-module",
                &rust_in,
                "--module",
                "module:Module1",
                "--source",
                &replace_source,
                "--dry-run",
            ],
            go_in.as_str(),
            rust_in.as_str(),
        ),
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "{label} exit");
        assert_eq!(rust_stdout, go_stdout, "{label} stdout");
        assert_eq!(
            scrub_path(
                rust_stderr.expect("rust error stderr"),
                scrub_rust,
                "[FILE]"
            ),
            scrub_path(go_stderr.expect("go error stderr"), scrub_go, "[FILE]"),
            "{label} stderr"
        );
    }

    let missing_source = temp_dir.join("Missing.bas").to_string_lossy().to_string();
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&[
        "--json",
        "vba",
        "add-module",
        &go_removed_module,
        "--source",
        &missing_source,
        "--out",
        &go_removed_module,
    ]);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&[
        "--json",
        "vba",
        "add-module",
        &rust_removed_module,
        "--source",
        &missing_source,
        "--out",
        &rust_removed_module,
    ]);
    assert_eq!(rust_code, go_code, "missing source exit");
    assert_eq!(rust_stdout, go_stdout, "missing source stdout");
    assert_eq!(rust_stderr, go_stderr, "missing source stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn vba_replace_module_refuses_office_shaped_project_metadata() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-vba-replace-office-shaped-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");

    let bin_path = temp_dir.join("office-shaped-vbaProject.bin");
    fs::write(
        &bin_path,
        synthetic_vba_project_bin_with_vba_project_stream(vec![0xCC; 32]),
    )
    .expect("write synthetic Office-shaped vbaProject.bin");
    let input_path = temp_dir.join("input.xlsx");
    let xlsm_path = temp_dir.join("macro-enabled.xlsm");
    let replacement_path = temp_dir.join("Module1.bas");
    let output_path = temp_dir.join("should-not-exist.xlsm");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input_path).expect("input");
    fs::write(
        &replacement_path,
        "Attribute VB_Name = \"Module1\"\r\nPublic Sub Replaced()\r\nEnd Sub\r\n",
    )
    .expect("replacement source");

    let bin = bin_path.to_string_lossy().to_string();
    let input = input_path.to_string_lossy().to_string();
    let xlsm = xlsm_path.to_string_lossy().to_string();
    let replacement = replacement_path.to_string_lossy().to_string();
    let output = output_path.to_string_lossy().to_string();

    let (code, _stdout, stderr) = run_ooxml(&[
        "--json", "vba", "attach", &input, "--bin", &bin, "--out", &xlsm,
    ]);
    assert_eq!(code, 0, "attach synthetic Office-shaped project");
    assert_eq!(stderr, None, "attach stderr");

    let (code, stdout, stderr) = run_ooxml(&["--json", "vba", "list", &xlsm]);
    assert_eq!(code, 0, "list exit");
    assert_eq!(stderr, None, "list stderr");
    let module_hash = module_sha256(&stdout.expect("list stdout"), "Module1");

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "vba",
        "replace-module",
        &xlsm,
        "--module",
        "module:Module1",
        "--source",
        &replacement,
        "--expect-sha256",
        &module_hash,
        "--allow-experimental-vba-source-rewrite",
        "--out",
        &output,
    ]);
    assert_ne!(
        code, 0,
        "replace-module should refuse Office-shaped project"
    );
    assert_eq!(stdout, None, "refusal should not write JSON stdout");
    let stderr = stderr.expect("replace-module stderr");
    assert!(
        stderr
            .to_string()
            .contains("version-dependent _VBA_PROJECT metadata"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        !output_path.exists(),
        "guarded replace-module must not write an output package"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

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

#[test]
fn vba_convert_xlsm_to_xlsx_alias_removes_macros() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-vba-convert-{}-{suffix}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&temp_dir).expect("temp dir");
    let bin_path = temp_dir.join("vbaProject.bin");
    fs::write(&bin_path, b"not-real-vba-but-nonempty").expect("write vbaProject.bin");
    let input_path = temp_dir.join("input.xlsx");
    let xlsm_path = temp_dir.join("macro-enabled.xlsm");
    let xlsx_path = temp_dir.join("converted.xlsx");
    fs::copy("testdata/xlsx/minimal-workbook/workbook.xlsx", &input_path).expect("input xlsx");

    let bin = bin_path.to_string_lossy().to_string();
    let input = input_path.to_string_lossy().to_string();
    let xlsm = xlsm_path.to_string_lossy().to_string();
    let xlsx = xlsx_path.to_string_lossy().to_string();

    let (attach_code, _attach_stdout, attach_stderr) = run_ooxml(&[
        "--json", "vba", "attach", &input, "--bin", &bin, "--out", &xlsm,
    ]);
    assert_eq!(attach_code, 0, "attach exit");
    assert_eq!(attach_stderr, None, "attach stderr");

    let (convert_code, convert_stdout, convert_stderr) =
        run_ooxml(&["--json", "convert", "xlsm-to-xlsx", &xlsm, "--out", &xlsx]);
    assert_eq!(convert_code, 0, "convert exit");
    assert_eq!(convert_stderr, None, "convert stderr");
    assert!(xlsx_path.exists(), "converted workbook should exist");
    let result = convert_stdout.expect("convert stdout");
    assert_eq!(result["result"]["action"], "remove");
    assert_eq!(result["result"]["macroEnabled"], false);
    assert_eq!(result["vba"]["macroEnabled"], false);
    assert_eq!(result["vba"]["hasVbaProject"], false);
    assert_eq!(result["conversion"]["alias"], "xlsm-to-xlsx");
    assert_eq!(result["conversion"]["implementation"], "vba remove");
    assert_eq!(result["conversion"]["input"], xlsm);
    assert_eq!(result["conversion"]["output"], xlsx);
    assert_eq!(result["conversion"]["sourceExtension"], ".xlsm");
    assert_eq!(result["conversion"]["targetExtension"], ".xlsx");
    assert_eq!(
        result["conversion"]["proofCommand"],
        format!("ooxml validate --strict {}", command_arg_for_test(&xlsx))
    );
    assert!(
        result["conversion"]["macroRemovalCommand"]
            .as_str()
            .expect("macro removal command")
            .contains("ooxml --json vba remove")
    );

    let (inspect_code, inspect_stdout, inspect_stderr) =
        run_ooxml(&["--json", "vba", "inspect", &xlsx]);
    assert_eq!(inspect_code, 0, "inspect converted exit");
    assert_eq!(inspect_stderr, None, "inspect converted stderr");
    let inspect = inspect_stdout.expect("inspect converted stdout");
    assert_eq!(inspect["vba"]["packageType"], "xlsx");
    assert_eq!(inspect["vba"]["macroEnabled"], false);
    assert_eq!(inspect["vba"]["hasVbaProject"], false);

    let (validate_code, _validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", &xlsx]);
    assert_eq!(validate_code, 0, "validate converted exit");
    assert_eq!(validate_stderr, None, "validate converted stderr");

    let _ = fs::remove_dir_all(&temp_dir);
}

fn module_sha256(list_result: &Value, name: &str) -> String {
    list_result["project"]["modules"]
        .as_array()
        .expect("modules array")
        .iter()
        .find(|module| module["name"].as_str() == Some(name))
        .and_then(|module| module["sha256"].as_str())
        .unwrap_or_else(|| panic!("module sha256 for {name}"))
        .to_string()
}

#[derive(Clone)]
struct SyntheticVbaModule {
    name: &'static str,
    stream_name: &'static str,
    kind: &'static str,
    source: &'static str,
}

#[derive(Clone)]
struct SyntheticCfbEntry {
    name: String,
    object_type: u8,
    left: u32,
    right: u32,
    child: u32,
    start_sector: u32,
    size: u64,
}

fn synthetic_vba_project_bin() -> Vec<u8> {
    synthetic_vba_project_bin_with_vba_project_stream(vec![0xCC, 0x61])
}

fn synthetic_vba_project_bin_with_vba_project_stream(vba_project_stream: Vec<u8>) -> Vec<u8> {
    let modules = vec![
        SyntheticVbaModule {
            name: "Module1",
            stream_name: "Module1",
            kind: "standard",
            source: "Attribute VB_Name = \"Module1\"\r\nPublic Sub HelloWorld()\r\nEnd Sub\r\n",
        },
        SyntheticVbaModule {
            name: "Class1",
            stream_name: "Class1",
            kind: "class",
            source: "Attribute VB_Name = \"Class1\"\r\nPublic Function Answer()\r\nAnswer = 42\r\nEnd Function\r\n",
        },
    ];
    let mut streams = BTreeMap::new();
    streams.insert(
        "VBA/dir".to_string(),
        compressed_vba_literals(&synthetic_dir_stream(&modules)),
    );
    streams.insert("VBA/_VBA_PROJECT".to_string(), vba_project_stream);
    for module in modules {
        streams.insert(
            format!("VBA/{}", module.stream_name),
            compressed_vba_literals(module.source.as_bytes()),
        );
    }
    synthetic_cfb(streams)
}

fn synthetic_dir_stream(modules: &[SyntheticVbaModule]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(vba_dir_record(0x0003, &le16(1252)));
    out.extend(vba_dir_record(0x000F, &le16(modules.len() as u16)));
    for module in modules {
        out.extend(vba_dir_record(0x0019, module.name.as_bytes()));
        out.extend(vba_dir_record(0x0047, &utf16le_bytes(module.name)));
        out.extend(vba_dir_record(0x001A, module.stream_name.as_bytes()));
        out.extend(vba_dir_record(0x0032, &utf16le_bytes(module.stream_name)));
        out.extend(vba_dir_record(0x0031, &le32(0)));
        if module.kind == "class" {
            out.extend(vba_dir_record(0x0022, &[]));
        } else {
            out.extend(vba_dir_record(0x0021, &[]));
        }
        out.extend(vba_dir_record(0x002B, &[]));
    }
    out.extend(vba_dir_record(0x0010, &[]));
    out
}

fn vba_dir_record(id: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(6 + payload.len());
    out.extend(id.to_le_bytes());
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

fn compressed_vba_literals(mut raw: &[u8]) -> Vec<u8> {
    let mut out = vec![0x01];
    while !raw.is_empty() {
        let literal_len = raw.len().min(3600);
        let literal_chunk = &raw[..literal_len];
        let mut chunk = Vec::new();
        let mut offset = 0;
        while offset < literal_chunk.len() {
            let n = (literal_chunk.len() - offset).min(8);
            chunk.push(0x00);
            chunk.extend(&literal_chunk[offset..offset + n]);
            offset += n;
        }
        let header = (chunk.len() as u16 - 1) | 0x3000 | 0x8000;
        out.extend(header.to_le_bytes());
        out.extend(chunk);
        raw = &raw[literal_len..];
    }
    out
}

fn synthetic_cfb(streams: BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    const SECTOR_SIZE: usize = 512;
    const NO_STREAM: u32 = 0xFFFF_FFFF;
    const END_OF_CHAIN: u32 = 0xFFFF_FFFE;
    const FAT_SECTOR: u32 = 0xFFFF_FFFD;

    let mut names = vec!["dir".to_string(), "_VBA_PROJECT".to_string()];
    let mut module_names = streams
        .keys()
        .filter_map(|path| path.strip_prefix("VBA/"))
        .filter(|name| *name != "dir" && *name != "_VBA_PROJECT")
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    module_names.sort();
    names.extend(module_names);

    let mut sectors = vec![vec![0; SECTOR_SIZE]];
    let mut entries = vec![
        SyntheticCfbEntry {
            name: "Root Entry".to_string(),
            object_type: 5,
            left: NO_STREAM,
            right: NO_STREAM,
            child: 1,
            start_sector: END_OF_CHAIN,
            size: 0,
        },
        SyntheticCfbEntry {
            name: "VBA".to_string(),
            object_type: 1,
            left: NO_STREAM,
            right: NO_STREAM,
            child: 2,
            start_sector: END_OF_CHAIN,
            size: 0,
        },
    ];
    for (idx, name) in names.iter().enumerate() {
        let data = streams
            .get(&format!("VBA/{name}"))
            .unwrap_or_else(|| panic!("missing synthetic stream {name}"));
        let start = sectors.len() as u32;
        let mut padded = data.clone();
        while !padded.len().is_multiple_of(SECTOR_SIZE) {
            padded.push(0);
        }
        for chunk in padded.chunks(SECTOR_SIZE) {
            sectors.push(chunk.to_vec());
        }
        let right = if idx < names.len() - 1 {
            entries.len() as u32 + 1
        } else {
            NO_STREAM
        };
        entries.push(SyntheticCfbEntry {
            name: name.clone(),
            object_type: 2,
            left: NO_STREAM,
            right,
            child: NO_STREAM,
            start_sector: start,
            size: data.len() as u64,
        });
    }

    let dir_start = sectors.len() as u32;
    let mut dir_data = Vec::new();
    for entry in &entries {
        dir_data.extend(cfb_directory_entry(entry));
    }
    while !dir_data.len().is_multiple_of(SECTOR_SIZE) {
        dir_data.push(0);
    }
    for chunk in dir_data.chunks(SECTOR_SIZE) {
        sectors.push(chunk.to_vec());
    }

    let mut fat = vec![END_OF_CHAIN; sectors.len()];
    fat[0] = FAT_SECTOR;
    for entry in &entries {
        if entry.object_type != 2 || entry.size == 0 {
            continue;
        }
        let count = (entry.size as usize).div_ceil(SECTOR_SIZE);
        for i in 0..count.saturating_sub(1) {
            fat[entry.start_sector as usize + i] = entry.start_sector + i as u32 + 1;
        }
    }
    for i in 0..(sectors.len() - dir_start as usize).saturating_sub(1) {
        fat[dir_start as usize + i] = dir_start + i as u32 + 1;
    }
    for (idx, value) in fat.iter().enumerate() {
        sectors[0][idx * 4..idx * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    for idx in fat.len()..SECTOR_SIZE / 4 {
        sectors[0][idx * 4..idx * 4 + 4].copy_from_slice(&NO_STREAM.to_le_bytes());
    }

    let mut header = vec![0; 512];
    header[..8].copy_from_slice(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]);
    header[24..26].copy_from_slice(&0x003E_u16.to_le_bytes());
    header[26..28].copy_from_slice(&0x0003_u16.to_le_bytes());
    header[28..30].copy_from_slice(&0xFFFE_u16.to_le_bytes());
    header[30..32].copy_from_slice(&9_u16.to_le_bytes());
    header[32..34].copy_from_slice(&6_u16.to_le_bytes());
    header[44..48].copy_from_slice(&1_u32.to_le_bytes());
    header[48..52].copy_from_slice(&dir_start.to_le_bytes());
    header[56..60].copy_from_slice(&0_u32.to_le_bytes());
    header[60..64].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
    header[68..72].copy_from_slice(&END_OF_CHAIN.to_le_bytes());
    header[76..80].copy_from_slice(&0_u32.to_le_bytes());
    for offset in (80..512).step_by(4) {
        header[offset..offset + 4].copy_from_slice(&NO_STREAM.to_le_bytes());
    }

    let mut out = header;
    for sector in sectors {
        out.extend(sector);
    }
    out
}

fn cfb_directory_entry(entry: &SyntheticCfbEntry) -> Vec<u8> {
    let mut out = vec![0; 128];
    let name_bytes = utf16le_bytes(&(entry.name.clone() + "\0"));
    out[..name_bytes.len()].copy_from_slice(&name_bytes);
    out[64..66].copy_from_slice(&(name_bytes.len() as u16).to_le_bytes());
    out[66] = entry.object_type;
    out[67] = 1;
    out[68..72].copy_from_slice(&entry.left.to_le_bytes());
    out[72..76].copy_from_slice(&entry.right.to_le_bytes());
    out[76..80].copy_from_slice(&entry.child.to_le_bytes());
    out[116..120].copy_from_slice(&entry.start_sector.to_le_bytes());
    out[120..124].copy_from_slice(&(entry.size as u32).to_le_bytes());
    out
}

fn utf16le_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

fn le16(value: u16) -> [u8; 2] {
    value.to_le_bytes()
}

fn le32(value: u32) -> [u8; 4] {
    value.to_le_bytes()
}
