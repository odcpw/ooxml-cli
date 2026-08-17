#[test]
fn pptx_animations_list_json_and_missing_path_match_rust_baseline() {
    assert_rust_baseline_match(&[
        "--json",
        "pptx",
        "animations",
        "list",
        "testdata/pptx/animations-synthetic/presentation.pptx",
    ]);
    assert_rust_baseline_match(&[
        "--json",
        "pptx",
        "animations",
        "list",
        "testdata/pptx/title-content/presentation.pptx",
    ]);

    let missing = std::env::temp_dir().join(format!(
        "ooxml-rust-missing-animations-{}-{}.pptx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let missing_str = missing.to_str().expect("missing path");
    assert_rust_baseline_match(&["--json", "pptx", "animations", "list", missing_str]);
}

#[test]
fn pptx_animations_mutations_match_rust_baseline_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-animations-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("animations temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "animations",
        "add",
        fixture,
        "--slide",
        "1",
        "--shape",
        "shape:2",
        "--effect",
        "appear",
        "--dry-run",
    ];
    assert_rust_baseline_match(&dry_run_args);

    let baseline_s1 = temp_dir.join("baseline-s1.pptx");
    let baseline_s2 = temp_dir.join("baseline-s2.pptx");
    let baseline_s3 = temp_dir.join("baseline-s3.pptx");
    let baseline_reordered = temp_dir.join("baseline-reordered.pptx");
    let baseline_removed = temp_dir.join("baseline-removed.pptx");
    let rust_s1 = temp_dir.join("rust-s1.pptx");
    let rust_s2 = temp_dir.join("rust-s2.pptx");
    let rust_s3 = temp_dir.join("rust-s3.pptx");
    let rust_reordered = temp_dir.join("rust-reordered.pptx");
    let rust_removed = temp_dir.join("rust-removed.pptx");

    let mut input_go = fixture.to_string();
    let mut input_rust = fixture.to_string();
    for (effect, baseline_out, rust_out) in [
        ("appear", &baseline_s1, &rust_s1),
        ("wipe", &baseline_s2, &rust_s2),
        ("fade", &baseline_s3, &rust_s3),
    ] {
        let baseline_out_str = baseline_out.to_str().expect("baseline animation output");
        let rust_out_str = rust_out.to_str().expect("rust animation output");
        let mut baseline_args = vec![
            "--json",
            "pptx",
            "animations",
            "add",
            input_go.as_str(),
            "--slide",
            "1",
            "--shape",
            "shape:2",
            "--effect",
            effect,
            "--out",
            baseline_out_str,
        ];
        let mut rust_args = vec![
            "--json",
            "pptx",
            "animations",
            "add",
            input_rust.as_str(),
            "--slide",
            "1",
            "--shape",
            "shape:2",
            "--effect",
            effect,
            "--out",
            rust_out_str,
        ];
        if effect == "wipe" {
            baseline_args.extend(["--direction", "up"]);
            rust_args.extend(["--direction", "up"]);
        }
        let (baseline_code, baseline_stdout, baseline_stderr) = run_ooxml_baseline(&baseline_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, baseline_code, "add {effect} exit");
        assert_eq!(rust_stderr, baseline_stderr, "add {effect} stderr");
        let rust_json = rust_stdout.expect("rust add stdout");
        assert_eq!(
            scrub_paths(
                rust_json.clone(),
                &[(rust_out_str, "[OUT]"), (input_rust.as_str(), "[IN]")]
            ),
            scrub_paths(
                baseline_stdout.expect("baseline add stdout"),
                &[(baseline_out_str, "[OUT]"), (input_go.as_str(), "[IN]")]
            ),
            "add {effect} stdout"
        );
        assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");
        input_go = baseline_out_str.to_string();
        input_rust = rust_out_str.to_string();
    }

    let (baseline_list_code, baseline_list_stdout, baseline_list_stderr) = run_ooxml_baseline(&[
        "--json",
        "pptx",
        "animations",
        "list",
        baseline_s3.to_str().expect("baseline s3"),
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "animations",
        "list",
        rust_s3.to_str().expect("rust s3"),
    ]);
    assert_eq!(rust_list_code, baseline_list_code, "list after add exit");
    assert_eq!(
        rust_list_stderr, baseline_list_stderr,
        "list after add stderr"
    );
    assert_eq!(
        rust_list_stdout.clone().expect("rust list after add"),
        baseline_list_stdout.expect("baseline list after add"),
        "list after add stdout"
    );
    let list_json = rust_list_stdout.expect("rust list json");
    let effects = list_json["slides"][0]["effects"]
        .as_array()
        .expect("animation effects");
    let order = [
        effects[2]["clickStepId"].as_i64().expect("third click id"),
        effects[1]["clickStepId"].as_i64().expect("second click id"),
        effects[0]["clickStepId"].as_i64().expect("first click id"),
    ]
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join(",");

    let baseline_reordered_str = baseline_reordered
        .to_str()
        .expect("baseline reorder output");
    let rust_reordered_str = rust_reordered.to_str().expect("rust reorder output");
    let baseline_reorder_args = [
        "--json",
        "pptx",
        "animations",
        "reorder",
        baseline_s3.to_str().expect("baseline s3 path"),
        "--slide",
        "1",
        "--order",
        order.as_str(),
        "--out",
        baseline_reordered_str,
    ];
    let rust_reorder_args = [
        "--json",
        "pptx",
        "animations",
        "reorder",
        rust_s3.to_str().expect("rust s3 path"),
        "--slide",
        "1",
        "--order",
        order.as_str(),
        "--out",
        rust_reordered_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, baseline_code, "reorder exit");
    assert_eq!(rust_stderr, baseline_stderr, "reorder stderr");
    let rust_reorder_json = rust_stdout.expect("rust reorder stdout");
    assert_eq!(
        scrub_paths(
            rust_reorder_json.clone(),
            &[
                (rust_reordered_str, "[OUT]"),
                (rust_s3.to_str().expect("rust s3 scrub"), "[IN]"),
            ]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline reorder stdout"),
            &[
                (baseline_reordered_str, "[OUT]"),
                (baseline_s3.to_str().expect("baseline s3 scrub"), "[IN]"),
            ]
        ),
        "reorder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_reorder_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_reorder_json, "validateCommand");

    let rust_reordered_list =
        run_ooxml(&["--json", "pptx", "animations", "list", rust_reordered_str])
            .1
            .expect("rust reordered list");
    let remove_id = rust_reordered_list["slides"][0]["effects"][1]["effectId"]
        .as_i64()
        .expect("middle effect id")
        .to_string();
    let baseline_removed_str = baseline_removed.to_str().expect("baseline removed output");
    let rust_removed_str = rust_removed.to_str().expect("rust removed output");
    let baseline_remove_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        baseline_reordered_str,
        "--slide",
        "1",
        "--effect-id",
        remove_id.as_str(),
        "--out",
        baseline_removed_str,
    ];
    let rust_remove_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        rust_reordered_str,
        "--slide",
        "1",
        "--effect-id",
        remove_id.as_str(),
        "--out",
        rust_removed_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_remove_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_remove_args);
    assert_eq!(rust_code, baseline_code, "remove exit");
    assert_eq!(rust_stderr, baseline_stderr, "remove stderr");
    let rust_remove_json = rust_stdout.expect("rust remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove_json.clone(),
            &[(rust_removed_str, "[OUT]"), (rust_reordered_str, "[IN]")]
        ),
        scrub_paths(
            baseline_stdout.expect("baseline remove stdout"),
            &[
                (baseline_removed_str, "[OUT]"),
                (baseline_reordered_str, "[IN]")
            ]
        ),
        "remove stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove_json, "validateCommand");

    let missing_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "1",
        "--effect-id",
        "9999",
        "--dry-run",
    ];
    assert_rust_baseline_match(&missing_args);

    let prune_dry_run = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--dry-run",
    ];
    assert_rust_baseline_match(&prune_dry_run);

    let baseline_pruned = temp_dir.join("baseline-pruned.pptx");
    let rust_pruned = temp_dir.join("rust-pruned.pptx");
    let baseline_pruned_str = baseline_pruned.to_str().expect("baseline pruned output");
    let rust_pruned_str = rust_pruned.to_str().expect("rust pruned output");
    let baseline_prune_args = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "4",
        "--out",
        baseline_pruned_str,
    ];
    let rust_prune_args = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "4",
        "--out",
        rust_pruned_str,
    ];
    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&baseline_prune_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_prune_args);
    assert_eq!(rust_code, baseline_code, "prune saved exit");
    assert_eq!(rust_stderr, baseline_stderr, "prune saved stderr");
    let rust_prune_json = rust_stdout.expect("rust prune stdout");
    assert_eq!(
        scrub_path(rust_prune_json.clone(), rust_pruned_str, "[OUT]"),
        scrub_path(
            baseline_stdout.expect("baseline prune stdout"),
            baseline_pruned_str,
            "[OUT]"
        ),
        "prune saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_prune_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_prune_json, "validateCommand");

    let (baseline_code, baseline_stdout, baseline_stderr) =
        run_ooxml_baseline(&["--json", "pptx", "animations", "list", baseline_pruned_str]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "pptx", "animations", "list", rust_pruned_str]);
    assert_eq!(rust_code, baseline_code, "prune readback exit");
    assert_eq!(rust_stderr, baseline_stderr, "prune readback stderr");
    assert_eq!(
        rust_stdout.expect("rust prune readback"),
        baseline_stdout.expect("baseline prune readback"),
        "prune readback stdout"
    );
}

