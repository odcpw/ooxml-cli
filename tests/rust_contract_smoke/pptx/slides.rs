#[test]
fn pptx_new_slide_set_text_reports_independent_bullet_paragraphs() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let output = std::env::temp_dir().join(format!(
        "ooxml-rust-new-slide-paragraphs-{}-{suffix}.pptx",
        std::process::id()
    ));
    let output_str = output.to_str().expect("paragraph output path");
    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "new-slide-from-layout",
        "testdata/pptx/multi-layout/presentation.pptx",
        "--layout",
        "Title and Content",
        "--set-text",
        "body=- Alpha\n- Beta\n\t* Nested",
        "--out",
        output_str,
    ]);
    assert_eq!(code, 0, "new-slide paragraph exit");
    assert_eq!(stderr, None, "new-slide paragraph stderr");
    let result = stdout.expect("new-slide paragraph stdout");
    assert_eq!(result["newSlideNumber"], 5);
    assert_rust_emitted_ooxml_command_succeeds(&result, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&result, "validateCommand");

    let (code, stdout, stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        output_str,
        "--slide",
        "5",
        "--target",
        "body",
        "--include-text",
    ]);
    assert_eq!(code, 0);
    assert_eq!(stderr, None);
    let paragraphs = stdout.expect("paragraph readback")["shapes"][0]["paragraphs"]
        .as_array()
        .cloned()
        .expect("paragraph array");
    assert_eq!(paragraphs.len(), 3);
    assert_eq!(paragraphs[0]["text"], "Alpha");
    assert_eq!(paragraphs[0]["bullet"], true);
    assert_eq!(paragraphs[2]["level"], 1);
    assert_strict_validate_succeeds(output_str, "new-slide paragraph output");
}

#[test]
fn pptx_clone_slide_clones_notes_part_and_backlink_like_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-clone-notes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx clone notes temp dir");

    let fixture = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let baseline_out = temp_dir.join("baseline-clone-notes.pptx");
    let rust_out = temp_dir.join("rust-clone-notes.pptx");
    let baseline_out_str = baseline_out.to_str().expect("baseline clone notes path");
    let rust_out_str = rust_out.to_str().expect("rust clone notes path");
    let baseline_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        baseline_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        rust_out_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "clone notes exit");
    assert_eq!(rust_stderr, baseline_stderr, "clone notes stderr");
    let rust_json = rust_stdout.expect("rust clone notes stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline clone notes stdout"),
            baseline_out_str,
            "[OUT]"
        ),
        "clone notes stdout"
    );

    let new_slide_uri = rust_json["newSlideUri"].as_str().expect("new slide URI");
    let notes_uri = rust_json["notesUri"].as_str().expect("cloned notes URI");
    assert_eq!(
        rust_json["destination"]["notesPartUri"],
        Value::String(notes_uri.to_string()),
        "destination readback should report cloned notes"
    );
    assert_eq!(rust_json["destination"]["notes"], true);

    let slide_rels = read_zip_string(&rust_out, &rels_part_for_uri(new_slide_uri));
    assert!(
        slide_rels.contains(PPTX_NOTES_REL_TYPE),
        "cloned slide notes rel"
    );
    assert!(
        slide_rels.contains(&relationship_target_between_parts(new_slide_uri, notes_uri)),
        "cloned slide should point at cloned notes part: {slide_rels}"
    );
    assert!(
        zip_entry_exists(&rust_out, notes_uri.trim_start_matches('/')),
        "cloned notes part should exist"
    );
    let notes_rels = read_zip_string(&rust_out, &rels_part_for_uri(notes_uri));
    assert!(
        notes_rels.contains(PPTX_SLIDE_REL_TYPE),
        "cloned notes backlink rel"
    );
    assert!(
        notes_rels.contains(&relationship_target_between_parts(notes_uri, new_slide_uri)),
        "cloned notes should link back to cloned slide: {notes_rels}"
    );

    assert_strict_validate_succeeds(rust_out_str, "clone notes output");
    assert_conformance_check_runs(rust_out_str, "clone notes output");
}

#[test]
fn pptx_import_merge_authoring_commands_match_rust_baseline_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-import-merge-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx import/merge temp dir");

    let target = "testdata/pptx/minimal-title/presentation.pptx";
    let notes_source = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let multi_source = "testdata/pptx/slide-assembly-multi/presentation.pptx";

    for (label, args) in [
        (
            "slides import-slide dry-run",
            vec![
                "--json",
                "pptx",
                "slides",
                "import-slide",
                target,
                "--source",
                target,
                "--slide",
                "1",
                "--dry-run",
            ],
        ),
        (
            "slides merge dry-run",
            vec![
                "--json",
                "pptx",
                "slides",
                "merge",
                target,
                target,
                "--dry-run",
            ],
        ),
        (
            "layouts import dry-run",
            vec![
                "--json",
                "pptx",
                "layouts",
                "import",
                target,
                "--source",
                target,
                "--layout",
                "1",
                "--dry-run",
            ],
        ),
        (
            "masters import dry-run",
            vec![
                "--json",
                "pptx",
                "masters",
                "import",
                target,
                "--source",
                target,
                "--master",
                "1",
                "--dry-run",
            ],
        ),
        (
            "slides import-slide missing source slide",
            vec![
                "--json",
                "pptx",
                "slides",
                "import-slide",
                target,
                "--source",
                target,
                "--slide",
                "99",
                "--dry-run",
            ],
        ),
        (
            "layouts import missing layout",
            vec![
                "--json",
                "pptx",
                "layouts",
                "import",
                target,
                "--source",
                target,
                "--layout",
                "99",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }

    let baseline_source = temp_dir.join("baseline-renamed-source.pptx");
    let rust_source = temp_dir.join("rust-renamed-source.pptx");
    let baseline_source_str = baseline_source
        .to_str()
        .expect("baseline renamed source path");
    let rust_source_str = rust_source.to_str().expect("rust renamed source path");
    let baseline_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        target,
        "--layout",
        "1",
        "--name",
        "WorkerOImportedTitle",
        "--out",
        baseline_source_str,
    ];
    let rust_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        target,
        "--layout",
        "1",
        "--name",
        "WorkerOImportedTitle",
        "--out",
        rust_source_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_rename_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_rename_args);
    assert_eq!(rust_code, baseline_code, "renamed import source exit");
    assert_eq!(rust_stderr, baseline_stderr, "renamed import source stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust renamed source stdout"),
            rust_source_str,
            "[SOURCE]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline renamed source stdout"),
            baseline_source_str,
            "[SOURCE]"
        ),
        "renamed import source stdout"
    );

    let baseline_import_slide = temp_dir.join("baseline-import-slide.pptx");
    let rust_import_slide = temp_dir.join("rust-import-slide.pptx");
    let baseline_import_slide_str = baseline_import_slide
        .to_str()
        .expect("baseline import-slide path");
    let rust_import_slide_str = rust_import_slide.to_str().expect("rust import-slide path");
    let baseline_args = [
        "--json",
        "pptx",
        "slides",
        "import-slide",
        target,
        "--source",
        notes_source,
        "--slide",
        "1",
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--notes-policy",
        "clone",
        "--out",
        baseline_import_slide_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "slides",
        "import-slide",
        target,
        "--source",
        notes_source,
        "--slide",
        "1",
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--notes-policy",
        "clone",
        "--out",
        rust_import_slide_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "slides import-slide saved exit");
    assert_eq!(
        rust_stderr, baseline_stderr,
        "slides import-slide saved stderr"
    );
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust import-slide stdout"),
            rust_import_slide_str,
            "[OUT]"
        ),
        scrub_path(
            baseline_stdout.expect("baseline import-slide stdout"),
            baseline_import_slide_str,
            "[OUT]"
        ),
        "slides import-slide saved stdout"
    );
    assert_rust_baseline_match(&["--json", "validate", "--strict", rust_import_slide_str]);

    let baseline_merge = temp_dir.join("baseline-merge.pptx");
    let rust_merge = temp_dir.join("rust-merge.pptx");
    let baseline_merge_str = baseline_merge.to_str().expect("baseline merge path");
    let rust_merge_str = rust_merge.to_str().expect("rust merge path");
    let baseline_args = [
        "--json",
        "pptx",
        "slides",
        "merge",
        target,
        multi_source,
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--out",
        baseline_merge_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "slides",
        "merge",
        target,
        multi_source,
        "--layout-policy",
        "import",
        "--theme-policy",
        "import",
        "--out",
        rust_merge_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "slides merge saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides merge saved stderr");
    let rust_merge_json = rust_stdout.expect("rust merge stdout");
    assert_eq!(
        scrub_paths(
            rust_merge_json.clone(),
            &[("rust-merge.pptx", "[OUT]"), (rust_merge_str, "[OUT]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline merge stdout"),
            &[
                ("baseline-merge.pptx", "[OUT]"),
                (baseline_merge_str, "[OUT]")
            ]
        ),
        "slides merge saved stdout"
    );
    assert_rust_baseline_match(&["--json", "validate", "--strict", rust_merge_str]);

    let baseline_layout = temp_dir.join("baseline-layout-import.pptx");
    let rust_layout = temp_dir.join("rust-layout-import.pptx");
    let baseline_layout_str = baseline_layout
        .to_str()
        .expect("baseline layout import path");
    let rust_layout_str = rust_layout.to_str().expect("rust layout import path");
    let baseline_args = [
        "--json",
        "pptx",
        "layouts",
        "import",
        target,
        "--source",
        baseline_source_str,
        "--layout",
        "WorkerOImportedTitle",
        "--theme-policy",
        "import",
        "--out",
        baseline_layout_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "layouts",
        "import",
        target,
        "--source",
        rust_source_str,
        "--layout",
        "WorkerOImportedTitle",
        "--theme-policy",
        "import",
        "--out",
        rust_layout_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "layouts import saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "layouts import saved stderr");
    let rust_layout_json = rust_stdout.expect("rust layouts import stdout");
    assert_eq!(
        scrub_paths(
            rust_layout_json.clone(),
            &[(rust_layout_str, "[OUT]"), (rust_source_str, "[SOURCE]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline layouts import stdout"),
            &[
                (baseline_layout_str, "[OUT]"),
                (baseline_source_str, "[SOURCE]")
            ]
        ),
        "layouts import saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_layout_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_layout_json, "validateCommand");

    let baseline_master = temp_dir.join("baseline-master-import.pptx");
    let rust_master = temp_dir.join("rust-master-import.pptx");
    let baseline_master_str = baseline_master
        .to_str()
        .expect("baseline master import path");
    let rust_master_str = rust_master.to_str().expect("rust master import path");
    let baseline_args = [
        "--json",
        "pptx",
        "masters",
        "import",
        target,
        "--source",
        baseline_source_str,
        "--master",
        "1",
        "--theme-policy",
        "import",
        "--out",
        baseline_master_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "masters",
        "import",
        target,
        "--source",
        rust_source_str,
        "--master",
        "1",
        "--theme-policy",
        "import",
        "--out",
        rust_master_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, baseline_code, "masters import saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "masters import saved stderr");
    let rust_master_json = rust_stdout.expect("rust masters import stdout");
    assert_eq!(
        scrub_paths(
            rust_master_json.clone(),
            &[(rust_master_str, "[OUT]"), (rust_source_str, "[SOURCE]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline masters import stdout"),
            &[
                (baseline_master_str, "[OUT]"),
                (baseline_source_str, "[SOURCE]")
            ]
        ),
        "masters import saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_master_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_master_json, "validateCommand");
}

#[test]
fn pptx_slides_lifecycle_saved_dry_run_readback_and_errors_match_rust_baseline() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-slides-lifecycle-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx slides lifecycle temp dir");

    let multi_fixture = "testdata/pptx/slide-assembly-multi/presentation.pptx";
    let notes_fixture = "testdata/pptx/notes-slide/presentation.pptx";

    let baseline_move = temp_dir.join("baseline-move.pptx");
    let rust_move = temp_dir.join("rust-move.pptx");
    let baseline_move_str = baseline_move.to_str().expect("baseline move path");
    let rust_move_str = rust_move.to_str().expect("rust move path");
    let baseline_move_args = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--out",
        baseline_move_str,
    ];
    let rust_move_args = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--out",
        rust_move_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_move_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_move_args);
    assert_eq!(rust_code, baseline_code, "slides move exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides move stderr");
    let rust_move_json = rust_stdout.expect("rust slides move stdout");
    assert_eq!(
        scrub_path(rust_move_json.clone(), rust_move_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline slides move stdout"),
            baseline_move_str,
            "[OUT]"
        ),
        "slides move stdout"
    );
    assert!(
        baseline_move.exists(),
        "Rust baseline slides move output missing"
    );
    assert!(rust_move.exists(), "Rust slides move output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_move_json, "validateCommand");
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", baseline_move_str],
        &["--json", "pptx", "slides", "list", rust_move_str],
        baseline_move_str,
        rust_move_str,
        "slides move readback list",
    );
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", baseline_move_str],
        &["--json", "validate", "--strict", rust_move_str],
        baseline_move_str,
        rust_move_str,
        "slides move strict validate",
    );

    let move_dry_run = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&move_dry_run, "slides move dry-run");

    let move_no_op_dry_run = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "2",
        "2",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&move_no_op_dry_run, "slides move no-op dry-run");

    let baseline_delete = temp_dir.join("baseline-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let baseline_delete_str = baseline_delete.to_str().expect("baseline delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let baseline_delete_args = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--out",
        baseline_delete_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--out",
        rust_delete_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, baseline_code, "slides delete exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides delete stderr");
    let rust_delete_json = rust_stdout.expect("rust slides delete stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline slides delete stdout"),
            baseline_delete_str,
            "[OUT]"
        ),
        "slides delete stdout"
    );
    assert!(
        baseline_delete.exists(),
        "Rust baseline slides delete output missing"
    );
    assert!(rust_delete.exists(), "Rust slides delete output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", baseline_delete_str],
        &["--json", "pptx", "slides", "list", rust_delete_str],
        baseline_delete_str,
        rust_delete_str,
        "slides delete readback list",
    );
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", baseline_delete_str],
        &["--json", "validate", "--strict", rust_delete_str],
        baseline_delete_str,
        rust_delete_str,
        "slides delete strict validate",
    );

    let delete_dry_run = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&delete_dry_run, "slides delete dry-run");

    let baseline_reorder = temp_dir.join("baseline-reorder.pptx");
    let rust_reorder = temp_dir.join("rust-reorder.pptx");
    let baseline_reorder_str = baseline_reorder.to_str().expect("baseline reorder path");
    let rust_reorder_str = rust_reorder.to_str().expect("rust reorder path");
    let baseline_reorder_args = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--out",
        baseline_reorder_str,
    ];
    let rust_reorder_args = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--out",
        rust_reorder_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, baseline_code, "slides reorder exit");
    assert_eq!(rust_stderr, baseline_stderr, "slides reorder stderr");
    let rust_reorder_json = rust_stdout.expect("rust slides reorder stdout");
    assert_eq!(
        scrub_path(rust_reorder_json.clone(), rust_reorder_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline slides reorder stdout"),
            baseline_reorder_str,
            "[OUT]"
        ),
        "slides reorder stdout"
    );
    assert!(
        baseline_reorder.exists(),
        "Rust baseline slides reorder output missing"
    );
    assert!(rust_reorder.exists(), "Rust slides reorder output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_reorder_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_reorder_json, "validateCommand");
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", baseline_reorder_str],
        &["--json", "pptx", "slides", "list", rust_reorder_str],
        baseline_reorder_str,
        rust_reorder_str,
        "slides reorder readback list",
    );
    assert_baseline_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", baseline_reorder_str],
        &["--json", "validate", "--strict", rust_reorder_str],
        baseline_reorder_str,
        rust_reorder_str,
        "slides reorder strict validate",
    );

    let reorder_dry_run = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--dry-run",
    ];
    assert_baseline_rust_json_match(&reorder_dry_run, "slides reorder dry-run");

    for (label, args) in [
        (
            "slides move from out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "move",
                multi_fixture,
                "9",
                "1",
                "--dry-run",
            ],
        ),
        (
            "slides move to out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "move",
                multi_fixture,
                "1",
                "9",
                "--dry-run",
            ],
        ),
        (
            "slides delete out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "delete",
                notes_fixture,
                "9",
                "--dry-run",
            ],
        ),
        (
            "slides reorder wrong length",
            vec![
                "--json",
                "pptx",
                "slides",
                "reorder",
                multi_fixture,
                "3,1,2",
                "--dry-run",
            ],
        ),
        (
            "slides reorder duplicate",
            vec![
                "--json",
                "pptx",
                "slides",
                "reorder",
                multi_fixture,
                "1,1,2,3,4",
                "--dry-run",
            ],
        ),
        (
            "slides reorder out-of-range",
            vec![
                "--json",
                "pptx",
                "slides",
                "reorder",
                multi_fixture,
                "9,1,2,3,4",
                "--dry-run",
            ],
        ),
    ] {
        assert_baseline_rust_json_match(&args, label);
    }
}
