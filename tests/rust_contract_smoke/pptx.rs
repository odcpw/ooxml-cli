// PPTX frozen mutation/render/verify contract tests live here while shared
// baseline and process helpers remain in the parent integration test crate.
use super::*;

fn assert_export_dirs_match(go_dir: &std::path::Path, rust_dir: &std::path::Path) {
    let go_files = sorted_export_files(go_dir);
    let rust_files = sorted_export_files(rust_dir);
    assert_eq!(rust_files, go_files, "exported file set");
    for relative in go_files {
        let go_bytes = std::fs::read(export_path(go_dir, &relative)).unwrap_or_else(|err| {
            panic!("read Go exported artifact {relative}: {err}");
        });
        let rust_bytes = std::fs::read(export_path(rust_dir, &relative)).unwrap_or_else(|err| {
            panic!("read Rust exported artifact {relative}: {err}");
        });
        assert_eq!(
            rust_bytes, go_bytes,
            "exported artifact bytes for {relative}"
        );
    }
}

fn export_path(root: &std::path::Path, relative: &str) -> std::path::PathBuf {
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        path.push(part);
    }
    path
}

fn sorted_export_files(root: &std::path::Path) -> Vec<String> {
    let mut files = Vec::new();
    collect_export_files(root, root, &mut files);
    files.sort();
    files
}

fn scrub_created_at(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            for (key, item) in map.iter_mut() {
                if key == "createdAt" && item.as_str().is_some() {
                    *item = Value::String("[CREATED_AT]".to_string());
                } else {
                    *item = scrub_created_at(item.take());
                }
            }
            Value::Object(map)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(scrub_created_at).collect()),
        other => other,
    }
}

fn collect_export_files(
    root: &std::path::Path,
    current: &std::path::Path,
    files: &mut Vec<String>,
) {
    for entry in std::fs::read_dir(current).unwrap_or_else(|err| {
        panic!("read export dir {}: {err}", current.display());
    }) {
        let path = entry.expect("export dir entry").path();
        if path.is_dir() {
            collect_export_files(root, &path, files);
        } else {
            let relative = path.strip_prefix(root).expect("relative export path");
            files.push(
                relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
    }
}

#[test]
fn pptx_add_textbox_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-add-textbox-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("add textbox temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "add-textbox dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "add-textbox dry-run stderr");
    assert_eq!(
        scrub_created_at(rust_stdout.expect("rust add-textbox dry-run")),
        scrub_created_at(go_stdout.expect("go add-textbox dry-run")),
        "add-textbox dry-run stdout"
    );

    let go_out = temp_dir.join("go-add-textbox.pptx");
    let rust_out = temp_dir.join("rust-add-textbox.pptx");
    let go_out_str = go_out.to_str().expect("go add-textbox output");
    let rust_out_str = rust_out.to_str().expect("rust add-textbox output");
    let go_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "add-textbox",
        fixture,
        "--slide",
        "1",
        "--text",
        "Agent text box",
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "2000000",
        "--cy",
        "500000",
        "--name",
        "Agent Box",
        "--font-size",
        "20",
        "--font",
        "Arial",
        "--bold",
        "--color",
        "FF0000",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "add-textbox saved exit");
    assert_eq!(rust_stderr, go_stderr, "add-textbox saved stderr");
    let rust_json = rust_stdout.expect("rust add-textbox saved");
    assert_eq!(
        scrub_created_at(scrub_path(rust_json.clone(), rust_out_str, "[OUT]")),
        scrub_created_at(scrub_path(
            go_stdout.expect("go add-textbox saved"),
            go_out_str,
            "[OUT]"
        )),
        "add-textbox saved stdout"
    );
    assert!(go_out.exists(), "Go add-textbox output missing");
    assert!(rust_out.exists(), "Rust add-textbox output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    for args in [
        vec![
            "--json",
            "pptx",
            "add-textbox",
            fixture,
            "--slide",
            "1",
            "--cx",
            "2000000",
            "--cy",
            "500000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "add-textbox",
            fixture,
            "--slide",
            "1",
            "--text",
            "Bad dimensions",
            "--cx",
            "0",
            "--cy",
            "500000",
            "--dry-run",
        ],
    ] {
        assert_go_rust_json_match(&args, "add-textbox representative error");
    }
}

#[test]
fn pptx_place_image_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-place-image-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("place image temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "place image dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "place image dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust place image dry-run"),
        go_stdout.expect("go place image dry-run"),
        "place image dry-run stdout"
    );

    let go_out = temp_dir.join("go-place-image.pptx");
    let rust_out = temp_dir.join("rust-place-image.pptx");
    let go_out_str = go_out.to_str().expect("go place image output");
    let rust_out_str = rust_out.to_str().expect("rust place image output");
    let go_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "place",
        "image",
        fixture,
        "--slide",
        "1",
        "--image",
        image,
        "--x",
        "100000",
        "--y",
        "200000",
        "--cx",
        "1000000",
        "--cy",
        "700000",
        "--name",
        "Agent Image",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "place image saved exit");
    assert_eq!(rust_stderr, go_stderr, "place image saved stderr");
    let rust_json = rust_stdout.expect("rust place image saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go place image saved"),
            go_out_str,
            "[OUT]"
        ),
        "place image saved stdout"
    );
    assert!(go_out.exists(), "Go place image output missing");
    assert!(rust_out.exists(), "Rust place image output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    for args in [
        vec![
            "--json",
            "pptx",
            "place",
            "image",
            fixture,
            "--slide",
            "1",
            "--image",
            "testdata/missing.png",
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "1",
            "--cy",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "place",
            "image",
            fixture,
            "--slide",
            "1",
            "--image",
            image,
            "--x",
            "0",
            "--y",
            "0",
            "--cx",
            "1",
            "--cy",
            "1",
            "--fit-mode",
            "stretch",
            "--dry-run",
        ],
    ] {
        assert_go_rust_json_match(&args, "place image representative error");
    }
}

#[test]
fn pptx_charts_list_show_json_and_errors_match_go_oracle() {
    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let list_args = ["--json", "pptx", "charts", "list", fixture];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&list_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&list_args);
    assert_eq!(rust_code, go_code, "charts list exit");
    assert_eq!(rust_stderr, go_stderr, "charts list stderr");
    let rust_list = rust_stdout.expect("rust charts list stdout");
    assert_eq!(
        rust_list,
        go_stdout.expect("go charts list stdout"),
        "charts list stdout"
    );
    assert_eq!(rust_list["charts"].as_array().map(Vec::len), Some(2));
    assert_eq!(rust_list["charts"][0]["primarySelector"], "chart:1");
    assert_eq!(
        rust_list["charts"][0]["style"]["title"]["text"],
        "Revenue by Region"
    );
    assert_rust_emitted_ooxml_command_exits_zero(&rust_list, "validateCommand");

    let second_show = rust_list["charts"][1]["showCommand"]
        .as_str()
        .expect("second chart showCommand");
    let emitted_args = emitted_ooxml_args(second_show);
    let emitted_borrowed = emitted_args.iter().map(String::as_str).collect::<Vec<_>>();
    let (show_code, show_stdout, show_stderr) = run_ooxml(&emitted_borrowed);
    assert_eq!(show_code, 0, "generated second chart show exit");
    assert_eq!(show_stderr, None, "generated second chart show stderr");
    let show_json = show_stdout.expect("generated second chart show stdout");
    assert_eq!(show_json["charts"].as_array().map(Vec::len), Some(1));
    assert_eq!(show_json["charts"][0]["partUri"], "/ppt/charts/chart2.xml");

    for args in [
        vec!["--json", "pptx", "charts", "list", fixture, "--slide", "2"],
        vec![
            "--json",
            "pptx",
            "charts",
            "show",
            fixture,
            "--slide",
            "1",
            "--chart",
            "Revenue Chart",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "show",
            fixture,
            "--chart",
            "part:/ppt/charts/chart2.xml",
        ],
    ] {
        let borrowed = args.to_vec();
        assert_go_rust_match(&borrowed);
    }

    for args in [
        vec!["--json", "pptx", "charts", "show", fixture],
        vec![
            "--json", "pptx", "charts", "show", fixture, "--chart", "missing",
        ],
        vec![
            "--json",
            "pptx",
            "charts",
            "show",
            "testdata/pptx/minimal-title/presentation.pptx",
        ],
    ] {
        let borrowed = args.to_vec();
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&borrowed);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&borrowed);
        assert_eq!(rust_code, go_code, "charts error exit for {borrowed:?}");
        assert_eq!(
            rust_stdout, go_stdout,
            "charts error stdout for {borrowed:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "charts error stderr for {borrowed:?}"
        );
    }
}

#[test]
fn pptx_animations_list_json_and_missing_path_match_go_oracle() {
    assert_go_rust_match(&[
        "--json",
        "pptx",
        "animations",
        "list",
        "testdata/pptx/animations-synthetic/presentation.pptx",
    ]);
    assert_go_rust_match(&[
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
    assert_go_rust_match(&["--json", "pptx", "animations", "list", missing_str]);
}

#[test]
fn pptx_animations_mutations_match_go_oracle_and_validate() {
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
    assert_go_rust_match(&dry_run_args);

    let go_s1 = temp_dir.join("go-s1.pptx");
    let go_s2 = temp_dir.join("go-s2.pptx");
    let go_s3 = temp_dir.join("go-s3.pptx");
    let go_reordered = temp_dir.join("go-reordered.pptx");
    let go_removed = temp_dir.join("go-removed.pptx");
    let rust_s1 = temp_dir.join("rust-s1.pptx");
    let rust_s2 = temp_dir.join("rust-s2.pptx");
    let rust_s3 = temp_dir.join("rust-s3.pptx");
    let rust_reordered = temp_dir.join("rust-reordered.pptx");
    let rust_removed = temp_dir.join("rust-removed.pptx");

    let mut input_go = fixture.to_string();
    let mut input_rust = fixture.to_string();
    for (effect, go_out, rust_out) in [
        ("appear", &go_s1, &rust_s1),
        ("wipe", &go_s2, &rust_s2),
        ("fade", &go_s3, &rust_s3),
    ] {
        let go_out_str = go_out.to_str().expect("go animation output");
        let rust_out_str = rust_out.to_str().expect("rust animation output");
        let mut go_args = vec![
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
            go_out_str,
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
            go_args.extend(["--direction", "up"]);
            rust_args.extend(["--direction", "up"]);
        }
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
        assert_eq!(rust_code, go_code, "add {effect} exit");
        assert_eq!(rust_stderr, go_stderr, "add {effect} stderr");
        let rust_json = rust_stdout.expect("rust add stdout");
        assert_eq!(
            scrub_paths(
                rust_json.clone(),
                &[(rust_out_str, "[OUT]"), (input_rust.as_str(), "[IN]")]
            ),
            scrub_paths(
                go_stdout.expect("go add stdout"),
                &[(go_out_str, "[OUT]"), (input_go.as_str(), "[IN]")]
            ),
            "add {effect} stdout"
        );
        assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");
        input_go = go_out_str.to_string();
        input_rust = rust_out_str.to_string();
    }

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "animations",
        "list",
        go_s3.to_str().expect("go s3"),
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "animations",
        "list",
        rust_s3.to_str().expect("rust s3"),
    ]);
    assert_eq!(rust_list_code, go_list_code, "list after add exit");
    assert_eq!(rust_list_stderr, go_list_stderr, "list after add stderr");
    assert_eq!(
        rust_list_stdout.clone().expect("rust list after add"),
        go_list_stdout.expect("go list after add"),
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

    let go_reordered_str = go_reordered.to_str().expect("go reorder output");
    let rust_reordered_str = rust_reordered.to_str().expect("rust reorder output");
    let go_reorder_args = [
        "--json",
        "pptx",
        "animations",
        "reorder",
        go_s3.to_str().expect("go s3 path"),
        "--slide",
        "1",
        "--order",
        order.as_str(),
        "--out",
        go_reordered_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, go_code, "reorder exit");
    assert_eq!(rust_stderr, go_stderr, "reorder stderr");
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
            go_stdout.expect("go reorder stdout"),
            &[
                (go_reordered_str, "[OUT]"),
                (go_s3.to_str().expect("go s3 scrub"), "[IN]"),
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
    let go_removed_str = go_removed.to_str().expect("go removed output");
    let rust_removed_str = rust_removed.to_str().expect("rust removed output");
    let go_remove_args = [
        "--json",
        "pptx",
        "animations",
        "remove",
        go_reordered_str,
        "--slide",
        "1",
        "--effect-id",
        remove_id.as_str(),
        "--out",
        go_removed_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_remove_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_remove_args);
    assert_eq!(rust_code, go_code, "remove exit");
    assert_eq!(rust_stderr, go_stderr, "remove stderr");
    let rust_remove_json = rust_stdout.expect("rust remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove_json.clone(),
            &[(rust_removed_str, "[OUT]"), (rust_reordered_str, "[IN]")]
        ),
        scrub_paths(
            go_stdout.expect("go remove stdout"),
            &[(go_removed_str, "[OUT]"), (go_reordered_str, "[IN]")]
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
    assert_go_rust_match(&missing_args);

    let prune_dry_run = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--dry-run",
    ];
    assert_go_rust_match(&prune_dry_run);

    let go_pruned = temp_dir.join("go-pruned.pptx");
    let rust_pruned = temp_dir.join("rust-pruned.pptx");
    let go_pruned_str = go_pruned.to_str().expect("go pruned output");
    let rust_pruned_str = rust_pruned.to_str().expect("rust pruned output");
    let go_prune_args = [
        "--json",
        "pptx",
        "animations",
        "prune-stale",
        "testdata/pptx/animations-synthetic/presentation.pptx",
        "--slide",
        "4",
        "--out",
        go_pruned_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_prune_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_prune_args);
    assert_eq!(rust_code, go_code, "prune saved exit");
    assert_eq!(rust_stderr, go_stderr, "prune saved stderr");
    let rust_prune_json = rust_stdout.expect("rust prune stdout");
    assert_eq!(
        scrub_path(rust_prune_json.clone(), rust_pruned_str, "[OUT]"),
        scrub_path(go_stdout.expect("go prune stdout"), go_pruned_str, "[OUT]"),
        "prune saved stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_prune_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_prune_json, "validateCommand");

    let (go_code, go_stdout, go_stderr) =
        run_go_ooxml(&["--json", "pptx", "animations", "list", go_pruned_str]);
    let (rust_code, rust_stdout, rust_stderr) =
        run_ooxml(&["--json", "pptx", "animations", "list", rust_pruned_str]);
    assert_eq!(rust_code, go_code, "prune readback exit");
    assert_eq!(rust_stderr, go_stderr, "prune readback stderr");
    assert_eq!(
        rust_stdout.expect("rust prune readback"),
        go_stdout.expect("go prune readback"),
        "prune readback stdout"
    );
}

#[test]
fn pptx_chart_style_mutations_match_go_oracle_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-chart-style-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("chart style temp dir");

    assert_pptx_chart_saved_mutation_matches_go(
        &temp_dir,
        "set-title",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--title",
            "Rust Title",
            "--expect-title",
            "Revenue by Region",
            "--font-family",
            "Arial",
            "--font-size",
            "14",
            "--font-color",
            "#112233",
            "--font-bold",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_go(
        &temp_dir,
        "set-legend",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--position",
            "bottom",
            "--overlay=false",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_go(
        &temp_dir,
        "set-plot-area-fill",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--fill-color",
            "#F3F6FA",
        ],
    );
    assert_pptx_chart_saved_mutation_matches_go(
        &temp_dir,
        "set-chart-area-fill",
        &["--slide", "1", "--chart", "chart:1", "--fill-color", "none"],
    );
    assert_pptx_chart_saved_mutation_matches_go(
        &temp_dir,
        "set-series-style",
        &[
            "--slide",
            "1",
            "--chart",
            "chart:1",
            "--series",
            "1",
            "--fill-color",
            "#AA5500",
            "--line-color",
            "#111111",
            "--line-width-pt",
            "2.25",
            "--expect-series-count",
            "1",
        ],
    );

    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "charts",
        "set-title",
        fixture,
        "--slide",
        "1",
        "--chart",
        "chart:1",
        "--title",
        "Dry Run Title",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "chart title dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "chart title dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust chart title dry-run stdout"),
        go_stdout.expect("go chart title dry-run stdout"),
        "chart title dry-run stdout"
    );
}

fn assert_pptx_chart_saved_mutation_matches_go(
    temp_dir: &Path,
    command: &str,
    extra_args: &[&str],
) {
    let fixture = "testdata/pptx/chart-simple/presentation.pptx";
    let go_out = temp_dir.join(format!("go-{command}.pptx"));
    let rust_out = temp_dir.join(format!("rust-{command}.pptx"));
    let go_out_str = go_out.to_str().expect("go chart mutation output path");
    let rust_out_str = rust_out.to_str().expect("rust chart mutation output path");

    let mut go_args = vec!["--json", "pptx", "charts", command, fixture];
    go_args.extend_from_slice(extra_args);
    go_args.extend_from_slice(&["--out", go_out_str]);
    let mut rust_args = vec!["--json", "pptx", "charts", command, fixture];
    rust_args.extend_from_slice(extra_args);
    rust_args.extend_from_slice(&["--out", rust_out_str]);

    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "{command} exit");
    assert_eq!(rust_stderr, go_stderr, "{command} stderr");
    let rust_json = rust_stdout.unwrap_or_else(|| panic!("rust {command} stdout"));
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.unwrap_or_else(|| panic!("go {command} stdout")),
            go_out_str,
            "[OUT]"
        ),
        "{command} stdout"
    );
    assert!(go_out.exists(), "Go {command} output missing");
    assert!(rust_out.exists(), "Rust {command} output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "chartShowCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        go_out_str,
        "--slide",
        "1",
        "--chart",
        "part:/ppt/charts/chart1.xml",
    ];
    let rust_show_args = [
        "--json",
        "pptx",
        "charts",
        "show",
        rust_out_str,
        "--slide",
        "1",
        "--chart",
        "part:/ppt/charts/chart1.xml",
    ];
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&go_show_args);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&rust_show_args);
    assert_eq!(rust_show_code, go_show_code, "{command} readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "{command} readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.unwrap_or_else(|| panic!("rust {command} readback")),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.unwrap_or_else(|| panic!("go {command} readback")),
            go_out_str,
            "[OUT]"
        ),
        "{command} readback stdout"
    );
}

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

#[test]
fn pptx_extract_images_artifacts_null_manifest_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-extract-images-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("extract images temp dir");

    let fixture = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let go_dir = temp_dir.join("go-images");
    let rust_dir = temp_dir.join("rust-images");
    let go_dir_str = go_dir.to_str().expect("go image dir");
    let rust_dir_str = rust_dir.to_str().expect("rust image dir");
    let go_args = [
        "--json", "pptx", "extract", "images", fixture, "--out", go_dir_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "extract",
        "images",
        fixture,
        "--out",
        rust_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "extract images exit");
    assert_eq!(rust_stderr, go_stderr, "extract images stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust extract images stdout"),
            rust_dir_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go extract images stdout"),
            go_dir_str,
            "[OUT]"
        ),
        "extract images stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

    let empty_fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let go_empty_dir = temp_dir.join("go-empty-images");
    let rust_empty_dir = temp_dir.join("rust-empty-images");
    let go_empty_dir_str = go_empty_dir.to_str().expect("go empty image dir");
    let rust_empty_dir_str = rust_empty_dir.to_str().expect("rust empty image dir");
    let go_empty_args = [
        "--json",
        "pptx",
        "extract",
        "images",
        empty_fixture,
        "--include-layout-images",
        "--out",
        go_empty_dir_str,
    ];
    let rust_empty_args = [
        "--json",
        "pptx",
        "extract",
        "images",
        empty_fixture,
        "--include-layout-images",
        "--out",
        rust_empty_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_empty_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_empty_args);
    assert_eq!(rust_code, go_code, "extract images empty exit");
    assert_eq!(rust_stderr, go_stderr, "extract images empty stderr");
    let rust_empty_json = rust_stdout.expect("rust empty images stdout");
    assert_eq!(rust_empty_json["images"], Value::Null);
    assert_eq!(
        scrub_path(rust_empty_json, rust_empty_dir_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go empty images stdout"),
            go_empty_dir_str,
            "[OUT]"
        ),
        "extract images empty stdout"
    );
    assert_export_dirs_match(&go_empty_dir, &rust_empty_dir);

    let out_of_range = [
        "--json",
        "pptx",
        "extract",
        "images",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "99",
        "--out",
        go_empty_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&out_of_range);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&out_of_range);
    assert_eq!(rust_code, go_code, "extract images out-of-range exit");
    assert_eq!(rust_stdout, go_stdout, "extract images out-of-range stdout");
    assert_eq!(rust_stderr, go_stderr, "extract images out-of-range stderr");
}

#[test]
fn pptx_media_list_json_filters_and_missing_args_match_go_oracle() {
    for (label, args) in [
        (
            "media list no media",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/minimal-title/presentation.pptx",
            ],
        ),
        (
            "media list synthetic",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-synthetic/presentation.pptx",
            ],
        ),
        (
            "media list stale",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-stale-media/presentation.pptx",
            ],
        ),
        (
            "media list slide filter",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-synthetic/presentation.pptx",
                "--slide",
                "5",
            ],
        ),
        (
            "media list missing slide filter",
            vec![
                "--json",
                "pptx",
                "media",
                "list",
                "testdata/pptx/animations-synthetic/presentation.pptx",
                "--slide",
                "99",
            ],
        ),
        (
            "media list missing file",
            vec!["--json", "pptx", "media", "list"],
        ),
        (
            "media add missing file",
            vec!["--json", "pptx", "media", "add"],
        ),
        (
            "media replace missing file",
            vec!["--json", "pptx", "media", "replace"],
        ),
    ] {
        assert_go_rust_json_match(&args, label);
    }
}

#[test]
fn pptx_media_add_replace_saved_readback_and_guards_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-media-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx media temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let clip = temp_dir.join("clip.mp4");
    let replacement_clip = temp_dir.join("replacement.mp4");
    std::fs::write(&clip, b"opaque-fake-media-bytes").expect("write media clip");
    std::fs::write(&replacement_clip, b"opaque-replacement-media-bytes")
        .expect("write replacement media clip");
    let clip_str = clip.to_str().expect("clip path");
    let replacement_clip_str = replacement_clip.to_str().expect("replacement clip path");

    let go_add_out = temp_dir.join("go-with-media.pptx");
    let rust_add_out = temp_dir.join("rust-with-media.pptx");
    let go_add_out_str = go_add_out.to_str().expect("go media add output");
    let rust_add_out_str = rust_add_out.to_str().expect("rust media add output");
    let go_add_args = [
        "--json",
        "pptx",
        "media",
        "add",
        fixture,
        "--slide",
        "1",
        "--file",
        clip_str,
        "--name",
        "Intro",
        "--out",
        go_add_out_str,
    ];
    let rust_add_args = [
        "--json",
        "pptx",
        "media",
        "add",
        fixture,
        "--slide",
        "1",
        "--file",
        clip_str,
        "--name",
        "Intro",
        "--out",
        rust_add_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, go_code, "media add exit");
    assert_eq!(rust_stderr, go_stderr, "media add stderr");
    let rust_add_json = rust_stdout.expect("rust media add stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_add_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go media add stdout"),
            go_add_out_str,
            "[OUT]"
        ),
        "media add stdout"
    );
    assert_eq!(rust_add_json["shapeName"], "Intro");
    assert_eq!(rust_add_json["kind"], "video");
    assert_eq!(rust_add_json["posterSynthesized"], Value::Bool(true));
    assert!(go_add_out.exists(), "Go media add output missing");
    assert!(rust_add_out.exists(), "Rust media add output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "media",
        "list",
        go_add_out_str,
        "--slide",
        "1",
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "media",
        "list",
        rust_add_out_str,
        "--slide",
        "1",
    ]);
    assert_eq!(rust_list_code, go_list_code, "media add readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "media add readback stderr"
    );
    assert_eq!(
        rust_list_stdout.expect("rust media add readback"),
        go_list_stdout.expect("go media add readback"),
        "media add readback stdout"
    );

    let go_replace_out = temp_dir.join("go-replaced-media.pptx");
    let rust_replace_out = temp_dir.join("rust-replaced-media.pptx");
    let go_replace_out_str = go_replace_out.to_str().expect("go media replace output");
    let rust_replace_out_str = rust_replace_out
        .to_str()
        .expect("rust media replace output");
    let shape_id = rust_add_json["shapeId"]
        .as_i64()
        .expect("media add shape id")
        .to_string();
    let go_replace_args = [
        "--json",
        "pptx",
        "media",
        "replace",
        go_add_out_str,
        "--slide",
        "1",
        "--shape",
        &shape_id,
        "--file",
        replacement_clip_str,
        "--expect-shape-name",
        "Intro",
        "--expect-media-kind",
        "video",
        "--out",
        go_replace_out_str,
    ];
    let rust_replace_args = [
        "--json",
        "pptx",
        "media",
        "replace",
        rust_add_out_str,
        "--slide",
        "1",
        "--shape",
        &shape_id,
        "--file",
        replacement_clip_str,
        "--expect-shape-name",
        "Intro",
        "--expect-media-kind",
        "video",
        "--out",
        rust_replace_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_replace_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_replace_args);
    assert_eq!(rust_code, go_code, "media replace exit");
    assert_eq!(rust_stderr, go_stderr, "media replace stderr");
    let rust_replace_json = rust_stdout.expect("rust media replace stdout");
    assert_eq!(
        scrub_paths(
            rust_replace_json.clone(),
            &[(rust_add_out_str, "[IN]"), (rust_replace_out_str, "[OUT]")]
        ),
        scrub_paths(
            go_stdout.expect("go media replace stdout"),
            &[(go_add_out_str, "[IN]"), (go_replace_out_str, "[OUT]")]
        ),
        "media replace stdout"
    );
    assert!(go_replace_out.exists(), "Go media replace output missing");
    assert!(
        rust_replace_out.exists(),
        "Rust media replace output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_replace_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_replace_json, "validateCommand");

    let guard_args = [
        "--json",
        "pptx",
        "media",
        "replace",
        rust_add_out_str,
        "--slide",
        "1",
        "--shape",
        &shape_id,
        "--file",
        replacement_clip_str,
        "--expect-shape-name",
        "WrongName",
        "--out",
        rust_replace_out_str,
    ];
    let (code, stdout, stderr) = run_ooxml(&guard_args);
    assert_eq!(code, 2, "media replace guard exit");
    assert_eq!(stdout, None, "media replace guard stdout");
    assert_eq!(
        stderr.expect("media replace guard stderr")["error"]["code"],
        "invalid_args"
    );
}

#[test]
fn pptx_replace_text_occurrences_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-occurrences-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace text occurrences temp dir");

    let fixture = "testdata/pptx/minimal-title/presentation.pptx";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace text occurrences dry-run exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences dry-run stderr"
    );
    let rust_dry_json = rust_stdout.expect("rust replace text occurrences dry-run");
    assert_eq!(
        rust_dry_json,
        go_stdout.expect("go replace text occurrences dry-run"),
        "replace text occurrences dry-run stdout"
    );
    let plan_hash = rust_dry_json["staleGuard"]["actualPlanHash"]
        .as_str()
        .expect("dry-run plan hash");

    let go_out = temp_dir.join("go-occurrences.pptx");
    let rust_out = temp_dir.join("rust-occurrences.pptx");
    let go_out_str = go_out.to_str().expect("go occurrences output path");
    let rust_out_str = rust_out.to_str().expect("rust occurrences output path");
    let go_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "1",
        "--expect-plan-hash",
        plan_hash,
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "1",
        "--expect-plan-hash",
        plan_hash,
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace text occurrences saved exit");
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences saved stderr"
    );
    let rust_json = rust_stdout.expect("rust replace text occurrences saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go replace text occurrences saved"),
            go_out_str,
            "[OUT]"
        ),
        "replace text occurrences saved stdout"
    );
    assert!(
        go_out.exists(),
        "Go replace text occurrences output missing"
    );
    assert!(
        rust_out.exists(),
        "Rust replace text occurrences output missing"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json["matches"][0], "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_text_code, go_text_stdout, go_text_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_text_code, rust_text_stdout, rust_text_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_text_code, go_text_code, "text readback exit");
    assert_eq!(rust_text_stderr, go_text_stderr, "text readback stderr");
    assert_eq!(
        scrub_path(
            rust_text_stdout.expect("rust text readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_text_stdout.expect("go text readback"),
            go_out_str,
            "[OUT]"
        ),
        "text readback stdout"
    );

    let count_mismatch = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Minimal",
        "--new-text",
        "Tiny",
        "--expect-count",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&count_mismatch);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&count_mismatch);
    assert_eq!(rust_code, go_code, "replace text occurrences guard exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace text occurrences guard stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences guard stderr"
    );

    let no_match = temp_dir.join("no-match.pptx");
    let no_match_str = no_match.to_str().expect("no match path");
    let no_match_args = [
        "--json",
        "pptx",
        "replace",
        "text-occurrences",
        fixture,
        "--match-text",
        "Missing",
        "--new-text",
        "Tiny",
        "--out",
        no_match_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&no_match_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&no_match_args);
    assert_eq!(rust_code, go_code, "replace text occurrences no-match exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace text occurrences no-match stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace text occurrences no-match stderr"
    );
}

#[test]
fn pptx_replace_images_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-images-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("replace images temp dir");

    let fixture = "testdata/pptx/slide-assembly-notes-media/presentation.pptx";
    let image = "testdata/test_image.png";
    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:4",
        "--image",
        image,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "replace images dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "replace images dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust replace images dry-run"),
        go_stdout.expect("go replace images dry-run"),
        "replace images dry-run stdout"
    );

    let go_out = temp_dir.join("go-image.pptx");
    let rust_out = temp_dir.join("rust-image.pptx");
    let go_out_str = go_out.to_str().expect("go image output path");
    let rust_out_str = rust_out.to_str().expect("rust image output path");
    let go_args = [
        "--json", "pptx", "replace", "images", fixture, "--slide", "2", "--target", "shape:4",
        "--image", image, "--out", go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:4",
        "--image",
        image,
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "replace images saved exit");
    assert_eq!(rust_stderr, go_stderr, "replace images saved stderr");
    let rust_json = rust_stdout.expect("rust replace images saved");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go replace images saved"),
            go_out_str,
            "[OUT]"
        ),
        "replace images saved stdout"
    );
    assert!(go_out.exists(), "Go replace images output missing");
    assert!(rust_out.exists(), "Rust replace images output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_dir = temp_dir.join("go-image-extract");
    let rust_dir = temp_dir.join("rust-image-extract");
    let go_dir_str = go_dir.to_str().expect("go image extract dir");
    let rust_dir_str = rust_dir.to_str().expect("rust image extract dir");
    let (go_extract_code, go_extract_stdout, go_extract_stderr) = run_go_ooxml(&[
        "--json", "pptx", "extract", "images", go_out_str, "--out", go_dir_str,
    ]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "extract",
        "images",
        rust_out_str,
        "--out",
        rust_dir_str,
    ]);
    assert_eq!(rust_extract_code, go_extract_code, "image readback exit");
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "image readback stderr"
    );
    assert_eq!(
        scrub_paths(
            rust_extract_stdout.expect("rust image readback"),
            &[(rust_out_str, "[PPTX]"), (rust_dir_str, "[OUT]")]
        ),
        scrub_paths(
            go_extract_stdout.expect("go image readback"),
            &[(go_out_str, "[PPTX]"), (go_dir_str, "[OUT]")]
        ),
        "image readback stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

    let missing_target = [
        "--json",
        "pptx",
        "replace",
        "images",
        fixture,
        "--slide",
        "2",
        "--target",
        "shape:999",
        "--image",
        image,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, go_code, "replace images missing target exit");
    assert_eq!(
        rust_stdout, go_stdout,
        "replace images missing target stdout"
    );
    assert_eq!(
        rust_stderr, go_stderr,
        "replace images missing target stderr"
    );
}

#[test]
fn pptx_extract_xml_artifacts_selectors_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-extract-xml-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("extract xml temp dir");

    let fixture = "testdata/pptx/multi-layout/presentation.pptx";
    let go_dir = temp_dir.join("go-xml");
    let rust_dir = temp_dir.join("rust-xml");
    let go_dir_str = go_dir.to_str().expect("go xml dir");
    let rust_dir_str = rust_dir.to_str().expect("rust xml dir");
    let go_args = [
        "--json", "pptx", "extract", "xml", fixture, "--slide", "1", "--layout", "1", "--master",
        "1", "--out", go_dir_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "extract",
        "xml",
        fixture,
        "--slide",
        "1",
        "--layout",
        "1",
        "--master",
        "1",
        "--out",
        rust_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "extract xml exit");
    assert_eq!(rust_stderr, go_stderr, "extract xml stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust extract xml stdout"),
            rust_dir_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go extract xml stdout"),
            go_dir_str,
            "[OUT]"
        ),
        "extract xml stdout"
    );
    assert_export_dirs_match(&go_dir, &rust_dir);

    let missing_out = [
        "--json",
        "pptx",
        "extract",
        "xml",
        "testdata/pptx/minimal-title/presentation.pptx",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_out);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_out);
    assert_eq!(rust_code, go_code, "extract xml missing out exit");
    assert_eq!(rust_stdout, go_stdout, "extract xml missing out stdout");
    assert_eq!(rust_stderr, go_stderr, "extract xml missing out stderr");

    let bad_layout_dir = temp_dir.join("bad-layout");
    let bad_layout_dir_str = bad_layout_dir.to_str().expect("bad layout dir");
    let bad_layout = [
        "--json",
        "pptx",
        "extract",
        "xml",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--layout",
        "99",
        "--out",
        bad_layout_dir_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&bad_layout);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&bad_layout);
    assert_eq!(rust_code, go_code, "extract xml bad layout exit");
    assert_eq!(rust_stdout, go_stdout, "extract xml bad layout stdout");
    assert_eq!(rust_stderr, go_stderr, "extract xml bad layout stderr");
}

#[test]
fn pptx_notes_set_clear_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-notes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("notes temp dir");

    let set_fixture = "testdata/pptx/title-content/presentation.pptx";
    let notes_fixture = "testdata/pptx/notes-slide/presentation.pptx";
    let go_set = temp_dir.join("go-set.pptx");
    let rust_set = temp_dir.join("rust-set.pptx");
    let go_set_str = go_set.to_str().expect("go set path");
    let rust_set_str = rust_set.to_str().expect("rust set path");
    let set_text = "First line\nSecond line";

    let go_set_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        set_text,
        "--out",
        go_set_str,
    ];
    let rust_set_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        set_text,
        "--out",
        rust_set_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, go_code, "notes set exit");
    assert_eq!(rust_stderr, go_stderr, "notes set stderr");
    let rust_set_json = rust_stdout.expect("rust notes set stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_set_str, "[OUT]"),
        scrub_path(go_stdout.expect("go notes set stdout"), go_set_str, "[OUT]"),
        "notes set stdout"
    );
    assert!(go_set.exists(), "Go notes set output missing");
    assert!(rust_set.exists(), "Rust notes set output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "notes", "show", go_set_str, "--slide", "1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        rust_set_str,
        "--slide",
        "1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "notes set readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "notes set readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust set readback"),
        go_show_stdout.expect("go set readback"),
        "notes set readback stdout"
    );

    let go_clear = temp_dir.join("go-clear.pptx");
    let rust_clear = temp_dir.join("rust-clear.pptx");
    let go_clear_str = go_clear.to_str().expect("go clear path");
    let rust_clear_str = rust_clear.to_str().expect("rust clear path");
    let go_clear_args = [
        "--json",
        "pptx",
        "notes",
        "clear",
        notes_fixture,
        "--slide",
        "2",
        "--out",
        go_clear_str,
    ];
    let rust_clear_args = [
        "--json",
        "pptx",
        "notes",
        "clear",
        notes_fixture,
        "--slide",
        "2",
        "--out",
        rust_clear_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_clear_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_clear_args);
    assert_eq!(rust_code, go_code, "notes clear exit");
    assert_eq!(rust_stderr, go_stderr, "notes clear stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust notes clear stdout"),
            rust_clear_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go notes clear stdout"),
            go_clear_str,
            "[OUT]"
        ),
        "notes clear stdout"
    );

    let (go_clear_show_code, go_clear_show_stdout, go_clear_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        go_clear_str,
        "--slide",
        "2",
    ]);
    let (rust_clear_show_code, rust_clear_show_stdout, rust_clear_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "notes",
        "show",
        rust_clear_str,
        "--slide",
        "2",
    ]);
    assert_eq!(
        rust_clear_show_code, go_clear_show_code,
        "notes clear readback exit"
    );
    assert_eq!(
        rust_clear_show_stderr, go_clear_show_stderr,
        "notes clear readback stderr"
    );
    assert_eq!(
        rust_clear_show_stdout.expect("rust clear readback"),
        go_clear_show_stdout.expect("go clear readback"),
        "notes clear readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "notes",
        "set",
        set_fixture,
        "--slide",
        "1",
        "--text",
        "draft notes",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "notes set dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "notes set dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust notes set dry-run"),
        go_stdout.expect("go notes set dry-run"),
        "notes set dry-run stdout"
    );

    let out_of_range = [
        "--json",
        "pptx",
        "notes",
        "set",
        "testdata/pptx/minimal-title/presentation.pptx",
        "--slide",
        "99",
        "--text",
        "x",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&out_of_range);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&out_of_range);
    assert_eq!(rust_code, go_code, "notes set out-of-range exit");
    assert_eq!(rust_stdout, go_stdout, "notes set out-of-range stdout");
    assert_eq!(rust_stderr, go_stderr, "notes set out-of-range stderr");
}

#[test]
fn pptx_shapes_get_set_bounds_delete_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-shapes-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx shapes temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let get_args = [
        "--json",
        "pptx",
        "shapes",
        "get",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&get_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&get_args);
    assert_eq!(rust_code, go_code, "shapes get exit");
    assert_eq!(rust_stderr, go_stderr, "shapes get stderr");
    assert_eq!(
        rust_stdout.expect("rust shapes get stdout"),
        go_stdout.expect("go shapes get stdout"),
        "shapes get stdout"
    );

    let go_bounds_out = temp_dir.join("go-set-bounds.pptx");
    let rust_bounds_out = temp_dir.join("rust-set-bounds.pptx");
    let go_bounds_out_str = go_bounds_out.to_str().expect("go set-bounds path");
    let rust_bounds_out_str = rust_bounds_out.to_str().expect("rust set-bounds path");
    let go_set_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        go_bounds_out_str,
    ];
    let rust_set_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        rust_bounds_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_set_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_set_args);
    assert_eq!(rust_code, go_code, "set-bounds saved exit");
    assert_eq!(rust_stderr, go_stderr, "set-bounds saved stderr");
    let rust_set_json = rust_stdout.expect("rust set-bounds stdout");
    assert_eq!(
        scrub_path(rust_set_json.clone(), rust_bounds_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go set-bounds stdout"),
            go_bounds_out_str,
            "[OUT]"
        ),
        "set-bounds saved stdout"
    );
    assert!(go_bounds_out.exists(), "Go set-bounds output missing");
    assert!(rust_bounds_out.exists(), "Rust set-bounds output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_set_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_set_json, "validateCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        go_bounds_out_str,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_bounds_out_str,
        "--slide",
        "2",
        "--target",
        "body",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(rust_read_code, go_read_code, "set-bounds readback exit");
    assert_eq!(
        rust_read_stderr, go_read_stderr,
        "set-bounds readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust set-bounds readback"),
            rust_bounds_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go set-bounds readback"),
            go_bounds_out_str,
            "[OUT]"
        ),
        "set-bounds readback stdout"
    );

    let set_dry_run_args = [
        "--json",
        "pptx",
        "shapes",
        "set-bounds",
        fixture,
        "--slide",
        "2",
        "--target",
        "body",
        "--bounds",
        "555555,666666,777777,888888",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&set_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&set_dry_run_args);
    assert_eq!(rust_code, go_code, "set-bounds dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "set-bounds dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-bounds dry-run stdout"),
        go_stdout.expect("go set-bounds dry-run stdout"),
        "set-bounds dry-run stdout"
    );

    let go_delete_out = temp_dir.join("go-delete-shape.pptx");
    let rust_delete_out = temp_dir.join("rust-delete-shape.pptx");
    let go_delete_out_str = go_delete_out.to_str().expect("go delete path");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete path");
    let go_delete_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--out",
        go_delete_out_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--out",
        rust_delete_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, go_code, "delete saved exit");
    assert_eq!(rust_stderr, go_stderr, "delete saved stderr");
    assert_eq!(
        scrub_path(
            rust_stdout.expect("rust delete stdout"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_stdout.expect("go delete stdout"),
            go_delete_out_str,
            "[OUT]"
        ),
        "delete saved stdout"
    );
    assert!(go_delete_out.exists(), "Go delete output missing");
    assert!(rust_delete_out.exists(), "Rust delete output missing");
    let (validate_code, validate_stdout, validate_stderr) =
        run_ooxml(&["--json", "validate", "--strict", rust_delete_out_str]);
    assert_eq!(validate_code, 0, "delete strict validate exit");
    assert_eq!(validate_stderr, None, "delete strict validate stderr");
    assert!(validate_stdout.is_some(), "delete strict validate stdout");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        go_delete_out_str,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "show",
        rust_delete_out_str,
        "--slide",
        "2",
        "--include-text",
        "--include-bounds",
    ]);
    assert_eq!(rust_show_code, go_show_code, "delete readback show exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "delete readback show stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete readback show"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go delete readback show"),
            go_delete_out_str,
            "[OUT]"
        ),
        "delete readback show stdout"
    );

    let delete_dry_run_args = [
        "--json",
        "pptx",
        "shapes",
        "delete",
        fixture,
        "--slide",
        "2",
        "--target",
        "title",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&delete_dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&delete_dry_run_args);
    assert_eq!(rust_code, go_code, "delete dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "delete dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust delete dry-run stdout"),
        go_stdout.expect("go delete dry-run stdout"),
        "delete dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json", "pptx", "shapes", "get", fixture, "--slide", "2", "--target", "missing",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "set-bounds",
            fixture,
            "--slide",
            "2",
            "--target",
            "missing",
            "--bounds",
            "1,2,3,4",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "set-bounds",
            fixture,
            "--slide",
            "2",
            "--target",
            "body",
            "--bounds",
            "bad",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "shapes",
            "delete",
            fixture,
            "--slide",
            "2",
            "--target",
            "missing",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "shape error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "shape error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "shape error stderr for {args:?}");
    }
}

#[test]
fn pptx_layouts_mutations_saved_readback_and_dry_run_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-layouts-mutation-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx layouts mutation temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";

    let go_rename = temp_dir.join("go-rename.pptx");
    let rust_rename = temp_dir.join("rust-rename.pptx");
    let go_rename_str = go_rename.to_str().expect("go rename path");
    let rust_rename_str = rust_rename.to_str().expect("rust rename path");
    let go_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "RustLayoutRenamed",
        "--out",
        go_rename_str,
    ];
    let rust_rename_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "RustLayoutRenamed",
        "--out",
        rust_rename_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_rename_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_rename_args);
    assert_eq!(rust_code, go_code, "layout rename exit");
    assert_eq!(rust_stderr, go_stderr, "layout rename stderr");
    let rust_rename_json = rust_stdout.expect("rust layout rename stdout");
    assert_eq!(
        scrub_path(rust_rename_json.clone(), rust_rename_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout rename stdout"),
            go_rename_str,
            "[OUT]"
        ),
        "layout rename stdout"
    );
    assert!(go_rename.exists(), "Go layout rename output missing");
    assert!(rust_rename.exists(), "Rust layout rename output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_rename_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_rename_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_rename_str,
        "--layout",
        "RustLayoutRenamed",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_rename_str,
        "--layout",
        "RustLayoutRenamed",
    ]);
    assert_eq!(rust_show_code, go_show_code, "layout rename readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout rename readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout rename readback"),
        go_show_stdout.expect("go layout rename readback"),
        "layout rename readback stdout"
    );

    let go_bounds = temp_dir.join("go-bounds.pptx");
    let rust_bounds = temp_dir.join("rust-bounds.pptx");
    let go_bounds_str = go_bounds.to_str().expect("go bounds path");
    let rust_bounds_str = rust_bounds.to_str().expect("rust bounds path");
    let go_bounds_args = [
        "--json",
        "pptx",
        "layouts",
        "set-bounds",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        go_bounds_str,
    ];
    let rust_bounds_args = [
        "--json",
        "pptx",
        "layouts",
        "set-bounds",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--bounds",
        "111111,222222,333333,444444",
        "--out",
        rust_bounds_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_bounds_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_bounds_args);
    assert_eq!(rust_code, go_code, "layout set-bounds exit");
    assert_eq!(rust_stderr, go_stderr, "layout set-bounds stderr");
    let rust_bounds_json = rust_stdout.expect("rust layout set-bounds stdout");
    assert_eq!(
        scrub_path(rust_bounds_json.clone(), rust_bounds_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout set-bounds stdout"),
            go_bounds_str,
            "[OUT]"
        ),
        "layout set-bounds stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_bounds_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_bounds_json, "validateCommand");
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_bounds_str,
        "--layout",
        "2",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_bounds_str,
        "--layout",
        "2",
    ]);
    assert_eq!(
        rust_show_code, go_show_code,
        "layout set-bounds readback exit"
    );
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout set-bounds readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout set-bounds readback"),
        go_show_stdout.expect("go layout set-bounds readback"),
        "layout set-bounds readback stdout"
    );

    let go_delete = temp_dir.join("go-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let go_delete_str = go_delete.to_str().expect("go delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let go_delete_args = [
        "--json",
        "pptx",
        "layouts",
        "delete-shape",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--out",
        go_delete_str,
    ];
    let rust_delete_args = [
        "--json",
        "pptx",
        "layouts",
        "delete-shape",
        fixture,
        "--layout",
        "2",
        "--target",
        "shape:3",
        "--out",
        rust_delete_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, go_code, "layout delete-shape exit");
    assert_eq!(rust_stderr, go_stderr, "layout delete-shape stderr");
    let rust_delete_json = rust_stdout.expect("rust layout delete-shape stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout delete-shape stdout"),
            go_delete_str,
            "[OUT]"
        ),
        "layout delete-shape stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_delete_str,
        "--layout",
        "2",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_delete_str,
        "--layout",
        "2",
    ]);
    assert_eq!(
        rust_show_code, go_show_code,
        "layout delete-shape readback exit"
    );
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout delete-shape readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout delete-shape readback"),
        go_show_stdout.expect("go layout delete-shape readback"),
        "layout delete-shape readback stdout"
    );

    let go_add = temp_dir.join("go-add-placeholder.pptx");
    let rust_add = temp_dir.join("rust-add-placeholder.pptx");
    let go_add_str = go_add.to_str().expect("go add-placeholder path");
    let rust_add_str = rust_add.to_str().expect("rust add-placeholder path");
    let go_add_args = [
        "--json",
        "pptx",
        "layouts",
        "add-placeholder",
        fixture,
        "--layout",
        "7",
        "--type",
        "pic",
        "--idx",
        "0",
        "--bounds",
        "1000,2000,3000,4000",
        "--out",
        go_add_str,
    ];
    let rust_add_args = [
        "--json",
        "pptx",
        "layouts",
        "add-placeholder",
        fixture,
        "--layout",
        "7",
        "--type",
        "pic",
        "--idx",
        "0",
        "--bounds",
        "1000,2000,3000,4000",
        "--out",
        rust_add_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, go_code, "layout add-placeholder exit");
    assert_eq!(rust_stderr, go_stderr, "layout add-placeholder stderr");
    let rust_add_json = rust_stdout.expect("rust layout add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_add_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout add-placeholder stdout"),
            go_add_str,
            "[OUT]"
        ),
        "layout add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");
    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "layouts", "show", go_add_str, "--layout", "7",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_add_str,
        "--layout",
        "7",
    ]);
    assert_eq!(
        rust_show_code, go_show_code,
        "layout add-placeholder readback exit"
    );
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout add-placeholder readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout add-placeholder readback"),
        go_show_stdout.expect("go layout add-placeholder readback"),
        "layout add-placeholder readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "layouts",
        "rename",
        fixture,
        "--layout",
        "2",
        "--name",
        "DryRunLayout",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "layout rename dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "layout rename dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust layout rename dry-run stdout"),
        go_stdout.expect("go layout rename dry-run stdout"),
        "layout rename dry-run stdout"
    );
}

#[test]
fn pptx_layout_slide_authoring_commands_match_go_oracle_and_validate() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-layout-slide-authoring-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx layout slide authoring temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";

    for args in [
        vec![
            "--json",
            "pptx",
            "layouts",
            "clone",
            fixture,
            "--layout",
            "1",
            "--name",
            "RustClonedLayout",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "masters",
            "add-placeholder",
            fixture,
            "--master",
            "1",
            "--type",
            "text",
            "--bounds",
            "100000,100000,1000000,500000",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "clone-slide",
            fixture,
            "--slide",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "new-slide-from-layout",
            fixture,
            "--layout",
            "1",
            "--set-text",
            "title=RustTitle",
            "--dry-run",
        ],
    ] {
        assert_go_rust_match(&args);
    }

    let go_layout = temp_dir.join("go-layout-clone.pptx");
    let rust_layout = temp_dir.join("rust-layout-clone.pptx");
    let go_layout_str = go_layout.to_str().expect("go layout clone path");
    let rust_layout_str = rust_layout.to_str().expect("rust layout clone path");
    let go_args = [
        "--json",
        "pptx",
        "layouts",
        "clone",
        fixture,
        "--layout",
        "1",
        "--name",
        "RustClonedLayout",
        "--out",
        go_layout_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "layouts",
        "clone",
        fixture,
        "--layout",
        "1",
        "--name",
        "RustClonedLayout",
        "--out",
        rust_layout_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "layout clone exit");
    assert_eq!(rust_stderr, go_stderr, "layout clone stderr");
    let rust_json = rust_stdout.expect("rust layout clone stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_layout_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go layout clone stdout"),
            go_layout_str,
            "[OUT]"
        ),
        "layout clone stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        go_layout_str,
        "--layout",
        "RustClonedLayout",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "layouts",
        "show",
        rust_layout_str,
        "--layout",
        "RustClonedLayout",
    ]);
    assert_eq!(rust_show_code, go_show_code, "layout clone readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "layout clone readback stderr"
    );
    assert_eq!(
        rust_show_stdout.expect("rust layout clone readback"),
        go_show_stdout.expect("go layout clone readback"),
        "layout clone readback stdout"
    );

    let go_master = temp_dir.join("go-master-placeholder.pptx");
    let rust_master = temp_dir.join("rust-master-placeholder.pptx");
    let go_master_str = go_master.to_str().expect("go master placeholder path");
    let rust_master_str = rust_master.to_str().expect("rust master placeholder path");
    let go_args = [
        "--json",
        "pptx",
        "masters",
        "add-placeholder",
        fixture,
        "--master",
        "1",
        "--type",
        "text",
        "--bounds",
        "100000,100000,1000000,500000",
        "--out",
        go_master_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "masters",
        "add-placeholder",
        fixture,
        "--master",
        "1",
        "--type",
        "text",
        "--bounds",
        "100000,100000,1000000,500000",
        "--out",
        rust_master_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "master add-placeholder exit");
    assert_eq!(rust_stderr, go_stderr, "master add-placeholder stderr");
    let rust_json = rust_stdout.expect("rust master add-placeholder stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_master_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go master add-placeholder stdout"),
            go_master_str,
            "[OUT]"
        ),
        "master add-placeholder stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_clone = temp_dir.join("go-clone-slide.pptx");
    let rust_clone = temp_dir.join("rust-clone-slide.pptx");
    let go_clone_str = go_clone.to_str().expect("go clone-slide path");
    let rust_clone_str = rust_clone.to_str().expect("rust clone-slide path");
    let go_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        go_clone_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "clone-slide",
        fixture,
        "--slide",
        "1",
        "--out",
        rust_clone_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "clone-slide exit");
    assert_eq!(rust_stderr, go_stderr, "clone-slide stderr");
    let rust_json = rust_stdout.expect("rust clone-slide stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_clone_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go clone-slide stdout"),
            go_clone_str,
            "[OUT]"
        ),
        "clone-slide stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let go_new = temp_dir.join("go-new-slide.pptx");
    let rust_new = temp_dir.join("rust-new-slide.pptx");
    let go_new_str = go_new.to_str().expect("go new slide path");
    let rust_new_str = rust_new.to_str().expect("rust new slide path");
    let go_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "1",
        "--set-text",
        "title=RustTitle",
        "--out",
        go_new_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "new-slide-from-layout",
        fixture,
        "--layout",
        "1",
        "--set-text",
        "title=RustTitle",
        "--out",
        rust_new_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "new-slide-from-layout exit");
    assert_eq!(rust_stderr, go_stderr, "new-slide-from-layout stderr");
    let rust_json = rust_stdout.expect("rust new-slide-from-layout stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_new_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go new-slide-from-layout stdout"),
            go_new_str,
            "[OUT]"
        ),
        "new-slide-from-layout stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let new_slide = rust_json["newSlideNumber"]
        .as_i64()
        .expect("new slide number");
    let new_slide_arg = new_slide.to_string();
    let rust_readback = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_new_str,
        "--slide",
        &new_slide_arg,
        "--target",
        "title",
        "--include-text",
    ])
    .1
    .expect("rust new slide title readback");
    assert_eq!(
        rust_readback["shapes"][0]["textPreview"], "RustTitle",
        "new slide title text readback"
    );
}

#[test]
fn pptx_slides_lifecycle_saved_dry_run_readback_and_errors_match_go_oracle() {
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

    let go_move = temp_dir.join("go-move.pptx");
    let rust_move = temp_dir.join("rust-move.pptx");
    let go_move_str = go_move.to_str().expect("go move path");
    let rust_move_str = rust_move.to_str().expect("rust move path");
    let go_move_args = [
        "--json",
        "pptx",
        "slides",
        "move",
        multi_fixture,
        "1",
        "3",
        "--out",
        go_move_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_move_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_move_args);
    assert_eq!(rust_code, go_code, "slides move exit");
    assert_eq!(rust_stderr, go_stderr, "slides move stderr");
    let rust_move_json = rust_stdout.expect("rust slides move stdout");
    assert_eq!(
        scrub_path(rust_move_json.clone(), rust_move_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go slides move stdout"),
            go_move_str,
            "[OUT]"
        ),
        "slides move stdout"
    );
    assert!(go_move.exists(), "Go slides move output missing");
    assert!(rust_move.exists(), "Rust slides move output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_succeeds(&rust_move_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_move_json, "validateCommand");
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", go_move_str],
        &["--json", "pptx", "slides", "list", rust_move_str],
        go_move_str,
        rust_move_str,
        "slides move readback list",
    );
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", go_move_str],
        &["--json", "validate", "--strict", rust_move_str],
        go_move_str,
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
    assert_go_rust_json_match(&move_dry_run, "slides move dry-run");

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
    assert_go_rust_json_match(&move_no_op_dry_run, "slides move no-op dry-run");

    let go_delete = temp_dir.join("go-delete.pptx");
    let rust_delete = temp_dir.join("rust-delete.pptx");
    let go_delete_str = go_delete.to_str().expect("go delete path");
    let rust_delete_str = rust_delete.to_str().expect("rust delete path");
    let go_delete_args = [
        "--json",
        "pptx",
        "slides",
        "delete",
        notes_fixture,
        "2",
        "--out",
        go_delete_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_delete_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_delete_args);
    assert_eq!(rust_code, go_code, "slides delete exit");
    assert_eq!(rust_stderr, go_stderr, "slides delete stderr");
    let rust_delete_json = rust_stdout.expect("rust slides delete stdout");
    assert_eq!(
        scrub_path(rust_delete_json.clone(), rust_delete_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go slides delete stdout"),
            go_delete_str,
            "[OUT]"
        ),
        "slides delete stdout"
    );
    assert!(go_delete.exists(), "Go slides delete output missing");
    assert!(rust_delete.exists(), "Rust slides delete output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_delete_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_delete_json, "validateCommand");
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", go_delete_str],
        &["--json", "pptx", "slides", "list", rust_delete_str],
        go_delete_str,
        rust_delete_str,
        "slides delete readback list",
    );
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", go_delete_str],
        &["--json", "validate", "--strict", rust_delete_str],
        go_delete_str,
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
    assert_go_rust_json_match(&delete_dry_run, "slides delete dry-run");

    let go_reorder = temp_dir.join("go-reorder.pptx");
    let rust_reorder = temp_dir.join("rust-reorder.pptx");
    let go_reorder_str = go_reorder.to_str().expect("go reorder path");
    let rust_reorder_str = rust_reorder.to_str().expect("rust reorder path");
    let go_reorder_args = [
        "--json",
        "pptx",
        "slides",
        "reorder",
        multi_fixture,
        "3,1,2,4,5",
        "--out",
        go_reorder_str,
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
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_reorder_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_reorder_args);
    assert_eq!(rust_code, go_code, "slides reorder exit");
    assert_eq!(rust_stderr, go_stderr, "slides reorder stderr");
    let rust_reorder_json = rust_stdout.expect("rust slides reorder stdout");
    assert_eq!(
        scrub_path(rust_reorder_json.clone(), rust_reorder_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go slides reorder stdout"),
            go_reorder_str,
            "[OUT]"
        ),
        "slides reorder stdout"
    );
    assert!(go_reorder.exists(), "Go slides reorder output missing");
    assert!(rust_reorder.exists(), "Rust slides reorder output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_reorder_json, "slidesListCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_reorder_json, "validateCommand");
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "pptx", "slides", "list", go_reorder_str],
        &["--json", "pptx", "slides", "list", rust_reorder_str],
        go_reorder_str,
        rust_reorder_str,
        "slides reorder readback list",
    );
    assert_go_rust_json_match_with_path_scrub(
        &["--json", "validate", "--strict", go_reorder_str],
        &["--json", "validate", "--strict", rust_reorder_str],
        go_reorder_str,
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
    assert_go_rust_json_match(&reorder_dry_run, "slides reorder dry-run");

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
        assert_go_rust_json_match(&args, label);
    }
}

fn assert_go_rust_json_match(args: &[&str], label: &str) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(rust_stdout, go_stdout, "{label} stdout");
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
}

fn assert_go_rust_json_match_with_path_scrub(
    go_args: &[&str],
    rust_args: &[&str],
    go_path: &str,
    rust_path: &str,
    label: &str,
) {
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(rust_args);
    assert_eq!(rust_code, go_code, "{label} exit");
    assert_eq!(
        scrub_path(rust_stdout.expect("rust stdout"), rust_path, "[OUT]"),
        scrub_path(go_stdout.expect("go stdout"), go_path, "[OUT]"),
        "{label} stdout"
    );
    assert_eq!(rust_stderr, go_stderr, "{label} stderr");
}

#[test]
fn pptx_tables_set_cell_saved_readback_dry_run_text_file_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-set-cell-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table set-cell temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let go_out = temp_dir.join("go-set-cell.pptx");
    let rust_out = temp_dir.join("rust-set-cell.pptx");
    let go_out_str = go_out.to_str().expect("go set-cell path");
    let rust_out_str = rust_out.to_str().expect("rust set-cell path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--row",
        "2",
        "--col",
        "2",
        "--text",
        "Rust Port Cell",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--row",
        "2",
        "--col",
        "2",
        "--text",
        "Rust Port Cell",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "set-cell saved exit");
    assert_eq!(rust_stderr, go_stderr, "set-cell saved stderr");
    let rust_json = rust_stdout.expect("rust set-cell stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(go_stdout.expect("go set-cell stdout"), go_out_str, "[OUT]"),
        "set-cell saved stdout"
    );
    assert!(go_out.exists(), "Go set-cell output missing");
    assert!(rust_out.exists(), "Rust set-cell output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "set-cell readback exit");
    assert_eq!(rust_show_stderr, go_show_stderr, "set-cell readback stderr");
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust set-cell readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go set-cell readback"),
            go_out_str,
            "[OUT]"
        ),
        "set-cell readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--col",
        "2",
        "--text",
        "Dry Cell",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "set-cell dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "set-cell dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-cell dry-run stdout"),
        go_stdout.expect("go set-cell dry-run stdout"),
        "set-cell dry-run stdout"
    );

    let text_file = temp_dir.join("cell-text.txt");
    std::fs::write(&text_file, "Text from file").expect("write set-cell text file");
    let text_file_str = text_file.to_str().expect("text file path");
    let text_file_args = [
        "--json",
        "pptx",
        "tables",
        "set-cell",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "1",
        "--col",
        "1",
        "--text-file",
        text_file_str,
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&text_file_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&text_file_args);
    assert_eq!(rust_code, go_code, "set-cell text-file dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "set-cell text-file dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust set-cell text-file dry-run stdout"),
        go_stdout.expect("go set-cell text-file dry-run stdout"),
        "set-cell text-file dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "0",
            "--col",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "9",
            "--col",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--row",
            "1",
            "--col",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "1",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "set-cell",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "1",
            "--col",
            "1",
            "--text",
            "x",
            "--text-file",
            text_file_str,
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "set-cell error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "set-cell error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "set-cell error stderr for {args:?}");
    }
}

#[test]
fn pptx_comments_add_edit_remove_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-comments-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx comments temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let go_added = temp_dir.join("go-added.pptx");
    let rust_added = temp_dir.join("rust-added.pptx");
    let go_added_str = go_added.to_str().expect("go added path");
    let rust_added_str = rust_added.to_str().expect("rust added path");

    let go_add_args = [
        "--json",
        "pptx",
        "comments",
        "add",
        fixture,
        "--slide",
        "1",
        "--author",
        "Alice",
        "--initials",
        "AB",
        "--text",
        "Fix the title",
        "--date",
        "2026-06-06T10:30:00Z",
        "--out",
        go_added_str,
    ];
    let rust_add_args = [
        "--json",
        "pptx",
        "comments",
        "add",
        fixture,
        "--slide",
        "1",
        "--author",
        "Alice",
        "--initials",
        "AB",
        "--text",
        "Fix the title",
        "--date",
        "2026-06-06T10:30:00Z",
        "--out",
        rust_added_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_add_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_add_args);
    assert_eq!(rust_code, go_code, "comments add exit");
    assert_eq!(rust_stderr, go_stderr, "comments add stderr");
    let go_add_json = go_stdout.expect("go comments add stdout");
    let rust_add_json = rust_stdout.expect("rust comments add stdout");
    assert_eq!(
        scrub_path(rust_add_json.clone(), rust_added_str, "[ADDED]"),
        scrub_path(go_add_json.clone(), go_added_str, "[ADDED]"),
        "comments add stdout"
    );
    assert!(go_added.exists(), "Go comments add output missing");
    assert!(rust_added.exists(), "Rust comments add output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_add_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_add_json, "validateCommand");

    let (go_list_code, go_list_stdout, go_list_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        go_added_str,
        "--slide",
        "1",
    ]);
    let (rust_list_code, rust_list_stdout, rust_list_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        rust_added_str,
        "--slide",
        "1",
    ]);
    assert_eq!(rust_list_code, go_list_code, "comments add readback exit");
    assert_eq!(
        rust_list_stderr, go_list_stderr,
        "comments add readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_list_stdout.expect("rust add readback"),
            rust_added_str,
            "[ADDED]"
        ),
        scrub_path(
            go_list_stdout.expect("go add readback"),
            go_added_str,
            "[ADDED]"
        ),
        "comments add readback stdout"
    );

    let handle = rust_add_json["handle"].as_str().expect("comment handle");
    let go_edited = temp_dir.join("go-edited.pptx");
    let rust_edited = temp_dir.join("rust-edited.pptx");
    let go_edited_str = go_edited.to_str().expect("go edited path");
    let rust_edited_str = rust_edited.to_str().expect("rust edited path");
    let go_edit_args = [
        "--json",
        "pptx",
        "comments",
        "edit",
        go_added_str,
        "--handle",
        handle,
        "--text",
        "Updated note",
        "--out",
        go_edited_str,
    ];
    let rust_edit_args = [
        "--json",
        "pptx",
        "comments",
        "edit",
        rust_added_str,
        "--handle",
        handle,
        "--text",
        "Updated note",
        "--out",
        rust_edited_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_edit_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_edit_args);
    assert_eq!(rust_code, go_code, "comments edit exit");
    assert_eq!(rust_stderr, go_stderr, "comments edit stderr");
    let rust_edit_json = rust_stdout.expect("rust comments edit stdout");
    assert_eq!(
        scrub_paths(
            rust_edit_json.clone(),
            &[(rust_added_str, "[ADDED]"), (rust_edited_str, "[EDITED]")]
        ),
        scrub_paths(
            go_stdout.expect("go comments edit stdout"),
            &[(go_added_str, "[ADDED]"), (go_edited_str, "[EDITED]")]
        ),
        "comments edit stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_edit_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_edit_json, "validateCommand");

    let go_removed = temp_dir.join("go-removed.pptx");
    let rust_removed = temp_dir.join("rust-removed.pptx");
    let go_removed_str = go_removed.to_str().expect("go removed path");
    let rust_removed_str = rust_removed.to_str().expect("rust removed path");
    let go_remove_args = [
        "--json",
        "pptx",
        "comments",
        "remove",
        go_edited_str,
        "--handle",
        handle,
        "--out",
        go_removed_str,
    ];
    let rust_remove_args = [
        "--json",
        "pptx",
        "comments",
        "remove",
        rust_edited_str,
        "--handle",
        handle,
        "--out",
        rust_removed_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_remove_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_remove_args);
    assert_eq!(rust_code, go_code, "comments remove exit");
    assert_eq!(rust_stderr, go_stderr, "comments remove stderr");
    let rust_remove_json = rust_stdout.expect("rust comments remove stdout");
    assert_eq!(
        scrub_paths(
            rust_remove_json.clone(),
            &[
                (rust_edited_str, "[EDITED]"),
                (rust_removed_str, "[REMOVED]")
            ]
        ),
        scrub_paths(
            go_stdout.expect("go comments remove stdout"),
            &[(go_edited_str, "[EDITED]"), (go_removed_str, "[REMOVED]")]
        ),
        "comments remove stdout"
    );
    assert_rust_emitted_ooxml_command_succeeds(&rust_remove_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_remove_json, "validateCommand");

    let (go_empty_code, go_empty_stdout, go_empty_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        go_removed_str,
        "--slide",
        "1",
    ]);
    let (rust_empty_code, rust_empty_stdout, rust_empty_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "comments",
        "list",
        rust_removed_str,
        "--slide",
        "1",
    ]);
    assert_eq!(
        rust_empty_code, go_empty_code,
        "comments remove readback exit"
    );
    assert_eq!(
        rust_empty_stderr, go_empty_stderr,
        "comments remove readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_empty_stdout.expect("rust remove readback"),
            rust_removed_str,
            "[REMOVED]"
        ),
        scrub_path(
            go_empty_stdout.expect("go remove readback"),
            go_removed_str,
            "[REMOVED]"
        ),
        "comments remove readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "comments",
        "add",
        fixture,
        "--slide",
        "1",
        "--author",
        "Alice",
        "--text",
        "Dry note",
        "--date",
        "2026-06-06T10:30:00Z",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "comments add dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "comments add dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust comments add dry-run"),
        go_stdout.expect("go comments add dry-run"),
        "comments add dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "comments",
            "add",
            fixture,
            "--slide",
            "99",
            "--author",
            "Alice",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "add",
            fixture,
            "--slide",
            "1",
            "--text",
            "x",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "edit",
            rust_added_str,
            "--slide",
            "1",
            "--comment-id",
            "1",
            "--text",
            "changed",
            "--expect-hash",
            "sha256:bogus",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "comments",
            "remove",
            fixture,
            "--slide",
            "1",
            "--comment-id",
            "1",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let go_args = args
            .iter()
            .map(|arg| {
                if *arg == rust_added_str {
                    go_added_str
                } else {
                    *arg
                }
            })
            .collect::<Vec<_>>();
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "comments error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "comments error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "comments error stderr for {args:?}");
    }
}

#[test]
fn pptx_tables_insert_row_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-insert-row-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table insert-row temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let merged_fixture = "testdata/pptx/table-merged/presentation.pptx";
    let go_out = temp_dir.join("go-insert-row.pptx");
    let rust_out = temp_dir.join("rust-insert-row.pptx");
    let go_out_str = go_out.to_str().expect("go insert-row path");
    let rust_out_str = rust_out.to_str().expect("rust insert-row path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "insert-row",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "2",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "insert-row",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "2",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "insert-row saved exit");
    assert_eq!(rust_stderr, go_stderr, "insert-row saved stderr");
    let rust_json = rust_stdout.expect("rust insert-row stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go insert-row stdout"),
            go_out_str,
            "[OUT]"
        ),
        "insert-row saved stdout"
    );
    assert!(go_out.exists(), "Go insert-row output missing");
    assert!(rust_out.exists(), "Rust insert-row output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "insert-row readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "insert-row readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust insert-row readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go insert-row readback"),
            go_out_str,
            "[OUT]"
        ),
        "insert-row readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "insert-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--at",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "insert-row dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "insert-row dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust insert-row dry-run stdout"),
        go_stdout.expect("go insert-row dry-run stdout"),
        "insert-row dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "0",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-row",
            merged_fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "3",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "insert-row error exit for {args:?}");
        assert_eq!(
            rust_stdout, go_stdout,
            "insert-row error stdout for {args:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "insert-row error stderr for {args:?}"
        );
    }
}

#[test]
fn pptx_tables_delete_row_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-delete-row-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let go_out = temp_dir.join("go-delete-row.pptx");
    let rust_out = temp_dir.join("rust-delete-row.pptx");
    let go_out_str = go_out.to_str().expect("go delete-row path");
    let rust_out_str = rust_out.to_str().expect("rust delete-row path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "delete-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "delete-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "delete-row saved exit");
    assert_eq!(rust_stderr, go_stderr, "delete-row saved stderr");
    let rust_json = rust_stdout.expect("rust delete-row stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go delete-row stdout"),
            go_out_str,
            "[OUT]"
        ),
        "delete-row saved stdout"
    );
    assert!(go_out.exists(), "Go delete-row output missing");
    assert!(rust_out.exists(), "Rust delete-row output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "delete-row readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "delete-row readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete-row readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go delete-row readback"),
            go_out_str,
            "[OUT]"
        ),
        "delete-row readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "delete-row",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--row",
        "2",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "delete-row dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "delete-row dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust delete-row dry-run stdout"),
        go_stdout.expect("go delete-row dry-run stdout"),
        "delete-row dry-run stdout"
    );

    for args in [
        [
            "--json",
            "pptx",
            "tables",
            "delete-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "99",
            "--dry-run",
        ],
        [
            "--json",
            "pptx",
            "tables",
            "delete-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--row",
            "0",
            "--dry-run",
        ],
        [
            "--json",
            "pptx",
            "tables",
            "delete-row",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--row",
            "1",
            "--dry-run",
        ],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "delete-row error exit for {args:?}");
        assert_eq!(
            rust_stdout, go_stdout,
            "delete-row error stdout for {args:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "delete-row error stderr for {args:?}"
        );
    }
}

#[test]
fn pptx_tables_column_saved_readback_dry_run_and_errors_match_go_oracle() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-table-cols-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx table column temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let merged_fixture = "testdata/pptx/table-merged/presentation.pptx";
    let go_insert_out = temp_dir.join("go-insert-col.pptx");
    let rust_insert_out = temp_dir.join("rust-insert-col.pptx");
    let go_insert_out_str = go_insert_out.to_str().expect("go insert-col path");
    let rust_insert_out_str = rust_insert_out.to_str().expect("rust insert-col path");

    let go_args = [
        "--json",
        "pptx",
        "tables",
        "insert-col",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "1",
        "--width-emu",
        "1234567",
        "--out",
        go_insert_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "insert-col",
        fixture,
        "--slide",
        "2",
        "--target",
        "table:1",
        "--at",
        "1",
        "--width-emu",
        "1234567",
        "--out",
        rust_insert_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "insert-col saved exit");
    assert_eq!(rust_stderr, go_stderr, "insert-col saved stderr");
    let rust_json = rust_stdout.expect("rust insert-col stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_insert_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go insert-col stdout"),
            go_insert_out_str,
            "[OUT]"
        ),
        "insert-col saved stdout"
    );
    assert!(go_insert_out.exists(), "Go insert-col output missing");
    assert!(rust_insert_out.exists(), "Rust insert-col output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        go_insert_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_insert_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "insert-col readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "insert-col readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust insert-col readback"),
            rust_insert_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go insert-col readback"),
            go_insert_out_str,
            "[OUT]"
        ),
        "insert-col readback stdout"
    );

    let go_delete_out = temp_dir.join("go-delete-col.pptx");
    let rust_delete_out = temp_dir.join("rust-delete-col.pptx");
    let go_delete_out_str = go_delete_out.to_str().expect("go delete-col path");
    let rust_delete_out_str = rust_delete_out.to_str().expect("rust delete-col path");
    let go_args = [
        "--json",
        "pptx",
        "tables",
        "delete-col",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--col",
        "2",
        "--out",
        go_delete_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "delete-col",
        fixture,
        "--slide",
        "2",
        "--table-id",
        "2",
        "--col",
        "2",
        "--out",
        rust_delete_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "delete-col saved exit");
    assert_eq!(rust_stderr, go_stderr, "delete-col saved stderr");
    let rust_json = rust_stdout.expect("rust delete-col stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_delete_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go delete-col stdout"),
            go_delete_out_str,
            "[OUT]"
        ),
        "delete-col saved stdout"
    );
    assert!(go_delete_out.exists(), "Go delete-col output missing");
    assert!(rust_delete_out.exists(), "Rust delete-col output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        go_delete_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_delete_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "delete-col readback exit");
    assert_eq!(
        rust_show_stderr, go_show_stderr,
        "delete-col readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust delete-col readback"),
            rust_delete_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go delete-col readback"),
            go_delete_out_str,
            "[OUT]"
        ),
        "delete-col readback stdout"
    );

    for dry_run_args in [
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "2",
            "--dry-run",
        ],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
        assert_eq!(
            rust_code, go_code,
            "column dry-run exit for {dry_run_args:?}"
        );
        assert_eq!(
            rust_stderr, go_stderr,
            "column dry-run stderr for {dry_run_args:?}"
        );
        assert_eq!(
            rust_stdout.expect("rust column dry-run stdout"),
            go_stdout.expect("go column dry-run stdout"),
            "column dry-run stdout for {dry_run_args:?}"
        );
    }

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "0",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--at",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "1",
            "--width-emu",
            "-1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "99",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "0",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            fixture,
            "--slide",
            "2",
            "--table-id",
            "99",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "delete-col",
            merged_fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--col",
            "1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "insert-col",
            merged_fixture,
            "--slide",
            "2",
            "--table-id",
            "2",
            "--at",
            "2",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "column error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "column error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "column error stderr for {args:?}");
    }
}

#[test]
fn pptx_tables_update_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-update-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx xlsx update temp dir");

    let fixture = "testdata/pptx/table-slide/presentation.pptx";
    let merged_fixture = "testdata/pptx/table-merged/presentation.pptx";
    let workbook = temp_dir.join("source-range.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_update_source_sheet_xml_4x4());
    let workbook_str = workbook.to_str().expect("source workbook path");
    let table_workbook = temp_dir.join("source-table.xlsx");
    write_pptx_update_table_xlsx(&table_workbook);
    let table_workbook_str = table_workbook.to_str().expect("source table workbook path");

    let go_out = temp_dir.join("go-update-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-update-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go update output path");
    let rust_out_str = rust_out.to_str().expect("rust update output path");
    let go_args = [
        "--json",
        "pptx",
        "tables",
        "update-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:C3",
        "--slide",
        "2",
        "--target",
        "table:1",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "tables",
        "update-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--formula-mode",
        "formula",
        "--expect-source-range",
        "A1:C3",
        "--slide",
        "2",
        "--target",
        "table:1",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "update-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "update-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust update-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go update-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "update-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go update-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust update-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_show_code, go_show_stdout, go_show_stderr) = run_go_ooxml(&[
        "--json", "pptx", "tables", "show", go_out_str, "--slide", "2", "--target", "table:1",
    ]);
    let (rust_show_code, rust_show_stdout, rust_show_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "tables",
        "show",
        rust_out_str,
        "--slide",
        "2",
        "--target",
        "table:1",
    ]);
    assert_eq!(rust_show_code, go_show_code, "update readback exit");
    assert_eq!(rust_show_stderr, go_show_stderr, "update readback stderr");
    assert_eq!(
        scrub_path(
            rust_show_stdout.expect("rust update readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_show_stdout.expect("go update readback"),
            go_out_str,
            "[OUT]"
        ),
        "update readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "tables",
        "update-from-xlsx",
        fixture,
        "--workbook",
        table_workbook_str,
        "--table",
        "Sales",
        "--slide",
        "2",
        "--target",
        "table:1",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "update-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "update-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust update-from-xlsx dry-run stdout"),
        go_stdout.expect("go update-from-xlsx dry-run stdout"),
        "update-from-xlsx dry-run stdout"
    );

    let title_content = "testdata/pptx/title-content/presentation.pptx";
    let bad_out = temp_dir.join("bad-update.pptx");
    let bad_out_str = bad_out.to_str().expect("bad update output path");
    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            table_workbook_str,
            "--range",
            "A1:C3",
            "--table",
            "Sales",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--range",
            "A1:C3",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--max-cells",
            "1",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--expect-source-range",
            "A1:B3",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:B2",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--formula-mode",
            "bad",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            merged_fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:D4",
            "--slide",
            "2",
            "--target",
            "table:1",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "tables",
            "update-from-xlsx",
            title_content,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1",
            "--slide",
            "2",
            "--target",
            "body",
            "--out",
            bad_out_str,
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "update error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "update error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "update error stderr for {args:?}");
    }
}

#[test]
fn pptx_replace_text_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-text-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx replace text xlsx temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("source-text.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_replace_text_source_sheet_xml());
    let workbook_str = workbook.to_str().expect("source workbook path");

    for args in [
        vec!["--json", "pptx", "replace", "text-from-xlsx"],
        vec!["--json", "pptx", "replace", "text-map-from-xlsx"],
    ] {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "missing file exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "missing file stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "missing file stderr for {args:?}");
    }

    let go_out = temp_dir.join("go-text-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go text-from-xlsx output path");
    let rust_out_str = rust_out.to_str().expect("rust text-from-xlsx output path");
    let go_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "title",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "title",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "text-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "text-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go text-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "text-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go text-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_succeeds(&rust_json, "readbackCommand");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_read_code, go_read_stdout, go_read_stderr) = run_go_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        go_out_str,
        "--slide",
        "1",
        "--target",
        "title",
        "--include-text",
    ]);
    let (rust_read_code, rust_read_stdout, rust_read_stderr) = run_ooxml(&[
        "--json",
        "pptx",
        "shapes",
        "get",
        rust_out_str,
        "--slide",
        "1",
        "--target",
        "title",
        "--include-text",
    ]);
    assert_eq!(rust_read_code, go_read_code, "text-from readback exit");
    assert_eq!(
        rust_read_stderr, go_read_stderr,
        "text-from readback stderr"
    );
    assert_eq!(
        scrub_path(
            rust_read_stdout.expect("rust text-from readback"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_read_stdout.expect("go text-from readback"),
            go_out_str,
            "[OUT]"
        ),
        "text-from readback stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "2",
        "--target",
        "body",
        "--row-sep",
        "\\n",
        "--col-sep",
        " | ",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "text-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "text-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-from-xlsx dry-run stdout"),
        go_stdout.expect("go text-from-xlsx dry-run stdout"),
        "text-from-xlsx dry-run stdout"
    );

    let missing_target = [
        "--json",
        "pptx",
        "replace",
        "text-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:B2",
        "--slide",
        "1",
        "--target",
        "missing",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&missing_target);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&missing_target);
    assert_eq!(rust_code, go_code, "text-from missing target exit");
    assert_eq!(rust_stdout, go_stdout, "text-from missing target stdout");
    assert_eq!(rust_stderr, go_stderr, "text-from missing target stderr");
}

#[test]
fn pptx_replace_text_map_from_xlsx_matches_go_oracle_saved_dry_run_and_errors() {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir().join(format!(
        "ooxml-rust-pptx-replace-text-map-xlsx-{}-{suffix}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("pptx replace text map xlsx temp dir");

    let fixture = "testdata/pptx/title-content/presentation.pptx";
    let workbook = temp_dir.join("source-map.xlsx");
    write_simple_xlsx_with_sheet_xml(&workbook, pptx_replace_text_map_source_sheet_xml());
    let workbook_str = workbook.to_str().expect("source map workbook path");
    let table_workbook = temp_dir.join("source-map-table.xlsx");
    write_pptx_text_map_table_xlsx(&table_workbook);
    let table_workbook_str = table_workbook.to_str().expect("source table workbook path");

    let go_out = temp_dir.join("go-text-map-from-xlsx.pptx");
    let rust_out = temp_dir.join("rust-text-map-from-xlsx.pptx");
    let go_out_str = go_out.to_str().expect("go text-map output path");
    let rust_out_str = rust_out.to_str().expect("rust text-map output path");
    let go_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--expect-source-range",
        "A1:C3",
        "--out",
        go_out_str,
    ];
    let rust_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        workbook_str,
        "--sheet",
        "Sheet1",
        "--range",
        "A1:C3",
        "--expect-source-range",
        "A1:C3",
        "--out",
        rust_out_str,
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&go_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&rust_args);
    assert_eq!(rust_code, go_code, "text-map-from-xlsx saved exit");
    assert_eq!(rust_stderr, go_stderr, "text-map-from-xlsx saved stderr");
    let rust_json = rust_stdout.expect("rust text-map-from-xlsx stdout");
    assert_eq!(
        scrub_path(rust_json.clone(), rust_out_str, "[OUT]"),
        scrub_path(
            go_stdout.expect("go text-map-from-xlsx stdout"),
            go_out_str,
            "[OUT]"
        ),
        "text-map-from-xlsx saved stdout"
    );
    assert!(go_out.exists(), "Go text-map-from-xlsx output missing");
    assert!(rust_out.exists(), "Rust text-map-from-xlsx output missing");
    assert_rust_emitted_ooxml_command_exits_zero(&rust_json, "validateCommand");

    let (go_extract_code, go_extract_stdout, go_extract_stderr) =
        run_go_ooxml(&["--json", "pptx", "extract", "text", go_out_str]);
    let (rust_extract_code, rust_extract_stdout, rust_extract_stderr) =
        run_ooxml(&["--json", "pptx", "extract", "text", rust_out_str]);
    assert_eq!(rust_extract_code, go_extract_code, "text-map extract exit");
    assert_eq!(
        rust_extract_stderr, go_extract_stderr,
        "text-map extract stderr"
    );
    assert_eq!(
        scrub_path(
            rust_extract_stdout.expect("rust text-map extract"),
            rust_out_str,
            "[OUT]"
        ),
        scrub_path(
            go_extract_stdout.expect("go text-map extract"),
            go_out_str,
            "[OUT]"
        ),
        "text-map extract stdout"
    );

    let dry_run_args = [
        "--json",
        "pptx",
        "replace",
        "text-map-from-xlsx",
        fixture,
        "--workbook",
        table_workbook_str,
        "--table",
        "TextMap",
        "--slide-col",
        "1",
        "--target-col",
        "2",
        "--text-col",
        "3",
        "--dry-run",
    ];
    let (go_code, go_stdout, go_stderr) = run_go_ooxml(&dry_run_args);
    let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&dry_run_args);
    assert_eq!(rust_code, go_code, "text-map-from-xlsx dry-run exit");
    assert_eq!(rust_stderr, go_stderr, "text-map-from-xlsx dry-run stderr");
    assert_eq!(
        rust_stdout.expect("rust text-map dry-run stdout"),
        go_stdout.expect("go text-map dry-run stdout"),
        "text-map-from-xlsx dry-run stdout"
    );

    let error_cases: Vec<Vec<&str>> = vec![
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--range",
            "A1:C3",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--slide-col",
            "1",
            "--target-col",
            "1",
            "--text-col",
            "3",
            "--dry-run",
        ],
        vec![
            "--json",
            "pptx",
            "replace",
            "text-map-from-xlsx",
            fixture,
            "--workbook",
            workbook_str,
            "--sheet",
            "Sheet1",
            "--range",
            "A1:C3",
            "--target-col",
            "3",
            "--text-col",
            "2",
            "--dry-run",
        ],
    ];
    for args in error_cases {
        let (go_code, go_stdout, go_stderr) = run_go_ooxml(&args);
        let (rust_code, rust_stdout, rust_stderr) = run_ooxml(&args);
        assert_eq!(rust_code, go_code, "text-map error exit for {args:?}");
        assert_eq!(rust_stdout, go_stdout, "text-map error stdout for {args:?}");
        assert_eq!(rust_stderr, go_stderr, "text-map error stderr for {args:?}");
    }
}

fn pptx_replace_text_source_sheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:B2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c></row>
  </sheetData>
</worksheet>"#
}

fn pptx_replace_text_map_source_sheet_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>Range Title</t></is></c></row>
    <row r="3"><c r="A3"><v>2</v></c><c r="B3" t="inlineStr"><is><t>body</t></is></c><c r="C3" t="inlineStr"><is><t>Range Body</t></is></c></row>
  </sheetData>
</worksheet>"#
}

fn pptx_update_source_sheet_xml_4x4() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:D4"/>
  <sheetData>
    <row r="1"><c r="A1"><f>SUM(B1:C1)</f><v>7</v></c><c r="B1" t="inlineStr"><is><t>Header B</t></is></c><c r="C1" t="inlineStr"><is><t>Header C</t></is></c><c r="D1" t="inlineStr"><is><t>D</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>North</t></is></c><c r="B2"><v>42</v></c><c r="C2" t="inlineStr"><is><t>ok</t></is></c><c r="D2" t="inlineStr"><is><t>H</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>South</t></is></c><c r="B3"><v>55</v></c><c r="C3" t="inlineStr"><is><t>done</t></is></c><c r="D3" t="inlineStr"><is><t>L</t></is></c></row>
    <row r="4"><c r="A4" t="inlineStr"><is><t>M</t></is></c><c r="B4" t="inlineStr"><is><t>N</t></is></c><c r="C4" t="inlineStr"><is><t>O</t></is></c><c r="D4" t="inlineStr"><is><t>P</t></is></c></row>
  </sheetData>
</worksheet>"#
}

fn write_pptx_update_table_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pptx update table xlsx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
</Types>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C3"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>Region</t></is></c><c r="B1" t="inlineStr"><is><t>Amount</t></is></c><c r="C1" t="inlineStr"><is><t>Note</t></is></c></row>
    <row r="2"><c r="A2" t="inlineStr"><is><t>East</t></is></c><c r="B2"><v>10</v></c><c r="C2" t="inlineStr"><is><t>ok</t></is></c></row>
    <row r="3"><c r="A3" t="inlineStr"><is><t>West</t></is></c><c r="B3"><f>SUM(B2:B2)*2</f><v>20</v></c><c r="C3" t="inlineStr"><is><t>done</t></is></c></row>
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/tables/table1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="Sales" displayName="Sales" ref="A1:C3" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:C3"/>
  <tableColumns count="3">
    <tableColumn id="1" name="Region"/>
    <tableColumn id="2" name="Amount"/>
    <tableColumn id="3" name="Note"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>"#,
    );
    writer.finish().expect("finish pptx update table xlsx");
}

fn write_pptx_text_map_table_xlsx(dest: &Path) {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).expect("fixture parent");
    }
    let output = File::create(dest).expect("create pptx text map table xlsx");
    let mut writer = ZipWriter::new(output);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    write_zip_string(
        &mut writer,
        options,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/tables/table1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml"/>
</Types>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Data" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/sheet1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C2"/>
  <sheetData>
    <row r="1"><c r="A1" t="inlineStr"><is><t>slide</t></is></c><c r="B1" t="inlineStr"><is><t>target</t></is></c><c r="C1" t="inlineStr"><is><t>text</t></is></c></row>
    <row r="2"><c r="A2"><v>1</v></c><c r="B2" t="inlineStr"><is><t>title</t></is></c><c r="C2" t="inlineStr"><is><t>Table Title</t></is></c></row>
  </sheetData>
  <tableParts count="1"><tablePart r:id="rId1"/></tableParts>
</worksheet>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/worksheets/_rels/sheet1.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
</Relationships>"#,
    );
    write_zip_string(
        &mut writer,
        options,
        "xl/tables/table1.xml",
        r#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" id="1" name="TextMap" displayName="TextMap" ref="A1:C2" headerRowCount="1" totalsRowShown="0">
  <autoFilter ref="A1:C2"/>
  <tableColumns count="3">
    <tableColumn id="1" name="slide"/>
    <tableColumn id="2" name="target"/>
    <tableColumn id="3" name="text"/>
  </tableColumns>
  <tableStyleInfo name="TableStyleMedium2" showFirstColumn="0" showLastColumn="0" showRowStripes="1" showColumnStripes="0"/>
</table>"#,
    );
    writer.finish().expect("finish pptx text map table xlsx");
}
